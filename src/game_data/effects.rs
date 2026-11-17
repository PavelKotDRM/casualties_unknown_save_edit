//! What using an item or a liquid does, read from the game's IL without
//! executing it.
//!
//! Item and liquid definitions (`Item.SetupItems`, the `Liquids` registry)
//! attach C# lambdas such as `useAction` and `onDrink`. Their bodies are
//! simple arithmetic on body and limb fields (`body.thirst += ml * 0.09f`),
//! calls such as `Body.Eat`, timed operations and minigame callbacks. This
//! module evaluates those bodies symbolically and reports the stat changes.
//! It is a best-effort reading: branches are not followed, so conditional
//! effects are listed together, and anything it cannot follow is skipped.

// FIXME: ~1100 lines; the public effect types, the IL reader and the symbolic value model all
// live in one file, and `Reader::analyze` is a ~220-line opcode dispatch with `Reader::call`
// another ~80. Split into `game_data/effects/{types.rs (Target, Change, Effect, ItemUse,
// ItemProfile, LiquidProfile, GameEffects), reader.rs (Reader, Definition, Callee, Call,
// find_method/callee/definitions/profiles), eval.rs (Lin, Obj, Val, arithmetic, is_closure and
// the index helpers), output.rs (RawChange, Raw, Out, merge)}`. Break `analyze` into per-family
// handlers (loads/stores, arithmetic, calls, branches) that take an explicit evaluation-state
// struct instead of threading `closure`/`depth` through every arm.

use std::collections::{BTreeMap, HashMap, HashSet};

use dotscope::CilObject;
use dotscope::assembly::{Immediate, Instruction, Operand};
use dotscope::metadata::method::MethodRc;
use dotscope::metadata::signatures::{SignatureMethod, TypeSignature};
use dotscope::metadata::tables::MemberRefSignature;
use dotscope::metadata::token::Token;
use dotscope::metadata::typesystem::CilTypeReference;

use super::{StaticValue, instruction_string, instruction_value};

/// Which object a change applies to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Target {
    /// The whole body.
    Body,
    /// The limb the item is used on.
    TreatedLimb,
    /// A specific limb by index into the body's limb array.
    Limb(usize),
    /// Every limb.
    EachLimb,
    /// A body component, such as `Painkillers`.
    Component(String),
}

/// How a stat changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Change {
    /// Adds the amount (negative values subtract).
    Add(f32),
    /// Multiplies the stat.
    Multiply(f32),
    /// Sets the stat to the value.
    Set(f32),
    /// An event without an amount, such as vomiting.
    Trigger,
}

/// One stat change.
#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    pub target: Target,
    /// Game field name (`pain`, `thirst`…) or a pseudo stat (`disinfect`, `vomit`).
    pub stat: String,
    pub change: Change,
    /// Applied gradually, once per second, over this many seconds.
    pub seconds: Option<f32>,
    /// The amount is for using up a whole item (in a minigame), rather than
    /// for a single use.
    pub whole_item: bool,
    /// Happens only under some condition (randomly, on overdose…); the
    /// amount is for one occurrence.
    pub conditional: bool,
}

/// How an item uses the liquid inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentsUse {
    Drink,
    Inject,
    ApplyToLimb,
}

/// What using an item does.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemUse {
    /// Used on a limb (`useLimbAction`) rather than on the body.
    pub on_limb: bool,
    pub effects: Vec<Effect>,
    /// Item condition used up by one use (1 = the whole item).
    pub condition_per_use: Option<f32>,
    /// Liquid contents used per use, in millilitres.
    pub contents: Option<(ContentsUse, f32)>,
}

/// Extra item data from `Item.SetupItems`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemProfile {
    /// Trade value.
    pub value: Option<f32>,
    /// Minutes until the item spoils.
    pub decay_minutes: Option<f32>,
    /// INT skill needed to recognise the item.
    pub recognition: Option<i32>,
    /// Crafting qualities with their amounts.
    pub qualities: Vec<(String, f32)>,
    /// Liquids the item starts with, in millilitres.
    pub default_contents: Vec<(String, f32)>,
    pub combineable: bool,
    /// Usable while depressed.
    pub ignore_depression: bool,
    pub untradeable: bool,
    /// Change of jump height while worn.
    pub jump_height_change: Option<f32>,
    pub uses: Vec<ItemUse>,
}

/// A liquid from the game's `Liquids.Registry`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiquidProfile {
    /// Key of the display name in the game's language files.
    pub locale_name: Option<String>,
    pub value_per_liter: Option<f32>,
    pub injectable: bool,
    /// Sickness caused per millilitre injected is `0.35 ×` this.
    pub injection_sickness: f32,
    pub qualities: Vec<(String, f32)>,
    /// Effects of drinking 100 ml.
    pub drink: Vec<Effect>,
    /// Effects of injecting 100 ml.
    pub inject: Vec<Effect>,
}

/// Effects of everything the player can use.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GameEffects {
    pub items: HashMap<String, ItemProfile>,
    pub liquids: BTreeMap<String, LiquidProfile>,
}

impl GameEffects {
    /// Effects of `ml` millilitres of an item's contents, mixed in the
    /// proportions the item starts with.
    pub fn contents_effects(&self, item: &str, ml: f32, inject: bool) -> Vec<Effect> {
        let Some(profile) = self.items.get(item) else {
            return Vec::new();
        };
        let total: f32 = profile.default_contents.iter().map(|(_, v)| v).sum();
        if total <= 0.0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (liquid, amount) in &profile.default_contents {
            let Some(liquid) = self.liquids.get(liquid) else {
                continue;
            };
            // FIXME: Liquid effects are normalized against a hard-coded 100 ml game-data baseline.
            let scale = ml * amount / total / 100.0;
            let effects = if inject {
                &liquid.inject
            } else {
                &liquid.drink
            };
            out.extend(effects.iter().map(|e| e.scaled(scale)));
        }
        merge(out)
    }
}

impl Effect {
    fn scaled(&self, factor: f32) -> Self {
        let change = match self.change {
            Change::Add(v) => Change::Add(v * factor),
            other => other,
        };
        Self {
            change,
            seconds: self.seconds.map(|s| s * factor),
            ..self.clone()
        }
    }
}

/// Reads item and liquid effects. Never fails: whatever cannot be read is left out.
pub(super) fn parse(assembly: &CilObject) -> GameEffects {
    let reader = Reader::new(assembly);
    let mut effects = GameEffects::default();
    if let Some(method) = reader.find_method("Item", "SetupItems") {
        for def in reader.definitions(&method, &["ItemInfo", "LiquidItemInfo"]) {
            effects
                .items
                .insert(def.id.clone(), reader.item_profile(&def));
        }
    }
    if let Some(method) = reader.find_method("Liquids", ".cctor") {
        for def in reader.definitions(&method, &["LiquidType"]) {
            effects
                .liquids
                .insert(def.id.clone(), reader.liquid_profile(&def));
        }
    }
    effects
}

/// One `new ItemInfo { … }` / `new LiquidType { … }` initialiser.
#[derive(Default)]
struct Definition {
    id: String,
    fields: HashMap<String, StaticValue>,
    lambdas: HashMap<String, Token>,
    qualities: Vec<(String, f32)>,
    contents: Vec<(String, f32)>,
    recognition: Option<i32>,
}

struct Callee {
    owner: String,
    name: String,
    /// Whether each parameter is a `float`.
    params: Vec<bool>,
    has_this: bool,
    returns: bool,
}

struct Reader<'a> {
    assembly: &'a CilObject,
    fields: HashMap<u32, (String, String)>,
}

impl<'a> Reader<'a> {
    fn new(assembly: &'a CilObject) -> Self {
        let mut fields = HashMap::new();
        for entry in assembly.types().iter() {
            let ty = entry.value();
            for field in ty.fields() {
                fields.insert(field.token.value(), (ty.name.clone(), field.name.clone()));
            }
        }
        Self { assembly, fields }
    }

    fn find_method(&self, owner: &str, name: &str) -> Option<MethodRc> {
        self.assembly.methods().iter().find_map(|entry| {
            let method = entry.value();
            let ty = method.declaring_type.get()?.upgrade()?;
            (ty.name == owner && method.name == name).then(|| method.clone())
        })
    }

    fn callee(&self, token: Token) -> Option<Callee> {
        let from_sig = |owner: String, name: String, sig: &SignatureMethod| Callee {
            owner,
            name,
            params: sig
                .params
                .iter()
                .map(|p| matches!(p.base, TypeSignature::R4))
                .collect(),
            has_this: sig.has_this,
            returns: !matches!(sig.return_type.base, TypeSignature::Void),
        };
        match token.table() {
            0x06 => {
                let entry = self.assembly.methods().get(&token)?;
                let method = entry.value();
                let owner = method
                    .declaring_type
                    .get()
                    .and_then(|r| r.upgrade())
                    .map(|t| t.name.clone())
                    .unwrap_or_default();
                Some(from_sig(owner, method.name.clone(), &method.signature))
            }
            0x0a => {
                let entry = self.assembly.refs_members().get(&token)?;
                let member = entry.value();
                let MemberRefSignature::Method(sig) = &member.signature else {
                    return None;
                };
                let owner = member.declaredby.name().unwrap_or_default();
                Some(from_sig(owner, member.name.clone(), sig))
            }
            0x2b => {
                let entry = self.assembly.method_specs().get(&token)?;
                match &entry.value().method {
                    CilTypeReference::MethodDef(method) => self.callee(method.upgrade()?.token),
                    CilTypeReference::MemberRef(member) => self.callee(member.token),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn field(&self, operand: &Operand) -> Option<&(String, String)> {
        let Operand::Token(token) = operand else {
            return None;
        };
        self.fields.get(&token.value())
    }

    /// Splits a definition method into the initialisers of `owners` types,
    /// keyed by the string just before `newobj` (`Add("id", new …)`).
    fn definitions(&self, method: &MethodRc, owners: &[&str]) -> Vec<Definition> {
        let strings = self.assembly.userstrings();
        let instructions: Vec<_> = method.instructions().collect();
        let mut list: Vec<Definition> = Vec::new();
        let mut pending_lambda = None;
        for (index, instruction) in instructions.iter().enumerate() {
            let previous = |back: usize| index.checked_sub(back).and_then(|i| instructions.get(i));
            match (instruction.mnemonic, &instruction.operand) {
                ("ldftn", Operand::Token(token)) => pending_lambda = Some(*token),
                ("newobj", Operand::Token(token)) => {
                    let Some(callee) = self.callee(*token) else {
                        continue;
                    };
                    if owners.contains(&callee.owner.as_str()) {
                        if let Some(id) = previous(1)
                            .filter(|p| p.mnemonic == "ldstr")
                            .and_then(|p| instruction_string(p, strings))
                        {
                            list.push(Definition {
                                id,
                                ..Default::default()
                            });
                        }
                        continue;
                    }
                    let Some(def) = list.last_mut() else {
                        continue;
                    };
                    let args: Vec<_> = (1..=callee.params.len())
                        .rev()
                        .map(|back| previous(back).and_then(|p| instruction_value(p, strings)))
                        .collect();
                    let text = |i: usize| match args.get(i) {
                        Some(Some(StaticValue::String(s))) => Some(s.clone()),
                        _ => None,
                    };
                    let number = |i: usize| match args.get(i) {
                        Some(Some(StaticValue::Number(n))) => Some(*n),
                        _ => None,
                    };
                    match callee.owner.as_str() {
                        "CraftingQuality" => {
                            if let Some(id) = text(0) {
                                def.qualities.push((id, number(1).unwrap_or(1.0)));
                            }
                        }
                        "LiquidStack" => {
                            if let (Some(id), Some(ml)) = (text(0), number(1)) {
                                def.contents.push((id, ml));
                            }
                        }
                        "Recognition" => def.recognition = number(0).map(|n| n as i32),
                        _ => {}
                    }
                }
                ("stfld", operand) => {
                    let (Some(def), Some((owner, name))) = (list.last_mut(), self.field(operand))
                    else {
                        continue;
                    };
                    if !owners.contains(&owner.as_str()) {
                        continue;
                    }
                    let is_lambda = matches!(
                        name.as_str(),
                        "useAction" | "useLimbAction" | "onDrink" | "onHealthUse"
                    );
                    if is_lambda {
                        if let Some(token) = pending_lambda.take() {
                            def.lambdas.insert(name.clone(), token);
                        }
                    } else if let Some(value) =
                        previous(1).and_then(|p| instruction_value(p, strings))
                    {
                        def.fields.insert(name.clone(), value);
                    }
                }
                _ => {}
            }
        }
        list
    }

    fn has_float_param(&self, token: Token) -> bool {
        self.callee(token).is_some_and(|c| c.params.contains(&true))
    }

    fn item_profile(&self, def: &Definition) -> ItemProfile {
        let number = |key: &str| super::number_field(&def.fields, key);
        let flag = |key: &str| super::bool_field(&def.fields, key).unwrap_or(false);
        let mut uses = Vec::new();
        for (field, on_limb) in [("useAction", false), ("useLimbAction", true)] {
            let Some(token) = def.lambdas.get(field) else {
                continue;
            };
            let mut closure = HashMap::new();
            let out = self
                .analyze(*token, &mut closure, 0)
                .normalize(self.has_float_param(*token));
            uses.push(ItemUse {
                on_limb,
                effects: out.effects(),
                condition_per_use: (out.condition.b < 0.0).then_some(-out.condition.b),
                contents: out.contents.first().map(|(kind, ml)| (*kind, ml.b)),
            });
        }
        ItemProfile {
            value: number("value"),
            decay_minutes: number("decayMinutes").filter(|m| *m > 0.0),
            recognition: def.recognition,
            qualities: def.qualities.clone(),
            default_contents: def.contents.clone(),
            combineable: flag("combineable"),
            ignore_depression: flag("ignoreDepression"),
            untradeable: flag("untradeable"),
            jump_height_change: number("jumpHeightMultChange").filter(|v| *v != 0.0),
            uses,
        }
    }

    fn liquid_profile(&self, def: &Definition) -> LiquidProfile {
        let per_100_ml = |field: &str| {
            def.lambdas.get(field).map_or_else(Vec::new, |token| {
                let mut closure = HashMap::new();
                self.analyze(*token, &mut closure, 0).at(100.0).effects()
            })
        };
        LiquidProfile {
            locale_name: super::string_field(&def.fields, "localeName").map(str::to_owned),
            value_per_liter: super::number_field(&def.fields, "valuePerLiter"),
            injectable: super::bool_field(&def.fields, "injectable").unwrap_or(false),
            injection_sickness: super::number_field(&def.fields, "injectionSickness")
                .unwrap_or(1.0),
            qualities: def.qualities.clone(),
            drink: per_100_ml("onDrink"),
            inject: per_100_ml("onHealthUse"),
        }
    }

    /// Symbolically runs one method. `closure` carries captured variables
    /// between a lambda and the lambdas it creates.
    fn analyze(&self, token: Token, closure: &mut HashMap<u32, Val>, depth: usize) -> Out {
        let mut out = Out::default();
        if depth > 4 || token.table() != 0x06 {
            return out;
        }
        let Some(entry) = self.assembly.methods().get(&token) else {
            return out;
        };
        let method = entry.value().clone();
        let signature = &method.signature;
        let strings = self.assembly.userstrings();
        let instructions: Vec<_> = method.instructions().collect();

        // Locals assigned more than once (loop counters, branches) are unknown.
        let mut stores: HashMap<u16, usize> = HashMap::new();
        for instruction in &instructions {
            if let Some(local) = local_index(instruction, "stloc") {
                *stores.entry(local).or_default() += 1;
            }
        }
        let mut locals: HashMap<u16, Val> = HashMap::new();
        let mut stack: Vec<Val> = Vec::new();
        let pop = |stack: &mut Vec<Val>| stack.pop().unwrap_or(Val::Unknown);

        // Effects between a conditional branch and its forward target only
        // happen sometimes. Unconditional jumps extend such a region (the
        // `else` part) but do not start one (loops start with a jump).
        let mut conditional_until = 0u64;
        for instruction in &instructions {
            let mnemonic = instruction.mnemonic;
            out.conditional = instruction.rva < conditional_until;
            if let Some(target) = instruction
                .branch_targets
                .iter()
                .copied()
                .filter(|target| *target > instruction.rva)
                .max()
            {
                let unconditional = matches!(mnemonic, "br" | "br.s" | "leave" | "leave.s");
                if !unconditional || out.conditional {
                    conditional_until = conditional_until.max(target);
                }
            }
            if let Some(arg) = arg_index(instruction) {
                let param = if signature.has_this {
                    arg.checked_sub(1)
                } else {
                    Some(arg)
                };
                let float = param
                    .and_then(|p| signature.params.get(usize::from(p)))
                    .is_some_and(|p| matches!(p.base, TypeSignature::R4));
                stack.push(if float {
                    Val::Num(Lin::X)
                } else {
                    Val::Obj(Obj::Any)
                });
                continue;
            }
            if let Some(local) = local_index(instruction, "ldloc") {
                let value = if stores.get(&local) == Some(&1) {
                    locals.get(&local).cloned().unwrap_or(Val::Unknown)
                } else {
                    Val::Unknown
                };
                stack.push(value);
                continue;
            }
            if let Some(local) = local_index(instruction, "stloc") {
                let value = pop(&mut stack);
                locals.insert(local, value);
                continue;
            }
            match mnemonic {
                "ldstr" | "ldnull" | "ldtoken" | "ldsfld" | "ldsflda" | "sizeof" => {
                    stack.push(Val::Unknown)
                }
                m if m.starts_with("ldc.") => {
                    stack.push(match instruction_value(instruction, strings) {
                        Some(StaticValue::Number(n)) => Val::Num(Lin::constant(n)),
                        _ => Val::Unknown,
                    })
                }
                m if m.starts_with("ldloca") || m.starts_with("ldarga") => {
                    stack.push(Val::Obj(Obj::Any))
                }
                "ldftn" => stack.push(match &instruction.operand {
                    Operand::Token(token) => Val::Func(*token),
                    _ => Val::Unknown,
                }),
                "ldvirtftn" => {
                    pop(&mut stack);
                    stack.push(Val::Unknown);
                }
                "dup" => {
                    let top = stack.last().cloned().unwrap_or(Val::Unknown);
                    stack.push(top);
                }
                "pop" | "stsfld" | "initobj" | "brtrue" | "brtrue.s" | "brfalse" | "brfalse.s"
                | "switch" => {
                    pop(&mut stack);
                }
                "ldfld" | "ldflda" => {
                    pop(&mut stack);
                    let Operand::Token(token) = &instruction.operand else {
                        stack.push(Val::Unknown);
                        continue;
                    };
                    let value = match self.fields.get(&token.value()) {
                        Some((owner, _)) if is_closure(owner) => {
                            closure.get(&token.value()).cloned().unwrap_or(Val::Unknown)
                        }
                        Some((owner, name)) if owner == "Body" && name == "limbs" => {
                            Val::Obj(Obj::Limbs)
                        }
                        Some(_) if mnemonic == "ldfld" => Val::Read(token.value()),
                        _ => Val::Obj(Obj::Any),
                    };
                    stack.push(value);
                }
                "stfld" => {
                    let value = pop(&mut stack);
                    let object = pop(&mut stack);
                    let Operand::Token(token) = &instruction.operand else {
                        continue;
                    };
                    let Some((owner, name)) = self.fields.get(&token.value()) else {
                        continue;
                    };
                    if is_closure(owner) {
                        closure.insert(token.value(), value);
                    } else {
                        out.store(owner, name, token.value(), &object, value);
                    }
                }
                m if m.starts_with("ldelem") => {
                    let index = pop(&mut stack);
                    let array = pop(&mut stack);
                    stack.push(match (array, index) {
                        (Val::Obj(Obj::Limbs), Val::Num(Lin { a, b })) if a == 0.0 && b >= 0.0 => {
                            Val::Obj(Obj::Limb(b as usize))
                        }
                        (Val::Obj(Obj::Limbs), _) => Val::Obj(Obj::EachLimb),
                        _ => Val::Unknown,
                    });
                }
                m if m.starts_with("stelem") => {
                    for _ in 0..3 {
                        pop(&mut stack);
                    }
                }
                "ldlen" | "box" | "unbox" | "unbox.any" | "castclass" | "isinst" | "newarr"
                | "not" => {
                    pop(&mut stack);
                    stack.push(Val::Unknown);
                }
                m if m.starts_with("conv.") => {
                    let value = pop(&mut stack);
                    stack.push(value);
                }
                "add" | "sub" | "mul" | "div" => {
                    let right = pop(&mut stack);
                    let left = pop(&mut stack);
                    stack.push(arithmetic(mnemonic, left, right));
                }
                "rem" | "and" | "or" | "xor" | "shl" | "shr" | "shr.un" | "ceq" | "cgt"
                | "cgt.un" | "clt" | "clt.un" => {
                    pop(&mut stack);
                    pop(&mut stack);
                    stack.push(Val::Unknown);
                }
                "neg" => {
                    let value = pop(&mut stack);
                    stack.push(match value {
                        Val::Num(n) => Val::Num(n.scale(-1.0)),
                        _ => Val::Unknown,
                    });
                }
                // Unconditional jumps end a statement; the rest compare two values.
                "br" | "br.s" | "ret" | "throw" | "leave" | "leave.s" | "endfinally" => {
                    stack.clear()
                }
                m if ["beq", "bne", "bge", "bgt", "ble", "blt"]
                    .iter()
                    .any(|p| m.starts_with(p)) =>
                {
                    pop(&mut stack);
                    pop(&mut stack);
                }
                "call" | "callvirt" | "newobj" => {
                    let Operand::Token(token) = &instruction.operand else {
                        continue;
                    };
                    let Some(callee) = self.callee(*token) else {
                        stack.clear();
                        continue;
                    };
                    let mut args: Vec<Val> =
                        (0..callee.params.len()).map(|_| pop(&mut stack)).collect();
                    args.reverse();
                    let is_new = mnemonic == "newobj";
                    let this = (callee.has_this && !is_new).then(|| pop(&mut stack));
                    let call = Call {
                        callee: &callee,
                        is_new,
                        this,
                        args,
                    };
                    if let Some(result) = self.call(call, closure, depth, &mut out) {
                        stack.push(result);
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Applies a call and returns its result, if it has one.
    fn call(
        &self,
        call: Call,
        closure: &mut HashMap<u32, Val>,
        depth: usize,
        out: &mut Out,
    ) -> Option<Val> {
        let Call {
            callee,
            is_new,
            this,
            args,
        } = call;
        let num = |i: usize| match args.get(i) {
            Some(Val::Num(n)) => Some(*n),
            _ => None,
        };
        match (callee.owner.as_str(), callee.name.as_str()) {
            ("Body", "Drink") => {
                if let Some(n) = num(0) {
                    out.add(Target::Body, "thirst", RawChange::Add(n));
                }
            }
            ("Body", "Eat") => {
                if let Some(n) = num(0) {
                    out.add(Target::Body, "hunger", RawChange::Add(n));
                }
                if let Some(n) = num(1) {
                    out.add(Target::Body, "weightOffset", RawChange::Add(n));
                }
            }
            ("WaterContainerItem", name @ ("Drink" | "Inject" | "ApplyToLimb")) => {
                let kind = match name {
                    "Drink" => ContentsUse::Drink,
                    "Inject" => ContentsUse::Inject,
                    _ => ContentsUse::ApplyToLimb,
                };
                // The amount parameter defaults to 100 ml.
                out.contents
                    .push((kind, num(1).unwrap_or(Lin::constant(100.0))));
            }
            ("Limb", "SetDisinfect") => {
                if let Some(n) = num(0) {
                    let target = match &this {
                        Some(Val::Obj(Obj::Limb(i))) => Target::Limb(*i),
                        Some(Val::Obj(Obj::EachLimb)) => Target::EachLimb,
                        _ => Target::TreatedLimb,
                    };
                    out.add(target, "disinfect", RawChange::Add(n));
                }
            }
            ("Vomiter", "Vomit") => out.add(Target::Body, "vomit", RawChange::Trigger),
            ("CoUtils", "DoTimedOp") => {
                if let (Some(Val::Func(func)), Some(duration)) = (args.get(1), num(2)) {
                    let inner = self.analyze(*func, closure, depth + 1);
                    out.merge_timed(inner, duration);
                }
                return None;
            }
            ("Mathf", "Clamp01" | "Clamp") => {
                return Some(args.into_iter().next().unwrap_or(Val::Unknown));
            }
            _ => {}
        }
        // A delegate constructor (`new Action(target, ftn)`) passes the lambda on.
        if is_new && args.len() == 2 && matches!(args[1], Val::Func(_)) {
            return Some(args[1].clone());
        }
        // Lambdas handed to anything else (minigames…) run as part of the use.
        for arg in &args {
            if let Val::Func(func) = arg {
                let inner = self
                    .analyze(*func, closure, depth + 1)
                    .normalize(self.has_float_param(*func));
                out.merge(inner);
            }
        }
        (is_new || callee.returns).then_some(Val::Obj(Obj::Any))
    }
}

struct Call<'a> {
    callee: &'a Callee,
    is_new: bool,
    this: Option<Val>,
    args: Vec<Val>,
}

fn is_closure(owner: &str) -> bool {
    owner.contains("DisplayClass")
}

fn arg_index(instruction: &Instruction) -> Option<u16> {
    let rest = instruction.mnemonic.strip_prefix("ldarg")?;
    if rest.starts_with('a') {
        return None;
    }
    match &instruction.operand {
        Operand::Argument(i) => Some(*i),
        Operand::Immediate(i) => immediate_index(i),
        _ => rest.strip_prefix('.').and_then(|n| n.parse().ok()),
    }
}

fn local_index(instruction: &Instruction, prefix: &str) -> Option<u16> {
    let rest = instruction.mnemonic.strip_prefix(prefix)?;
    if rest.starts_with('a') {
        return None;
    }
    match &instruction.operand {
        Operand::Local(i) => Some(*i),
        Operand::Immediate(i) => immediate_index(i),
        _ => rest.strip_prefix('.').and_then(|n| n.parse().ok()),
    }
}

fn immediate_index(immediate: &Immediate) -> Option<u16> {
    match immediate {
        Immediate::UInt8(v) => Some(u16::from(*v)),
        Immediate::Int8(v) => u16::try_from(*v).ok(),
        Immediate::UInt16(v) => Some(*v),
        Immediate::Int16(v) => u16::try_from(*v).ok(),
        _ => None,
    }
}

/// `a·x + b`, where `x` is the method's float parameter (millilitres,
/// minigame progress…).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Lin {
    a: f32,
    b: f32,
}

impl Lin {
    const X: Self = Self { a: 1.0, b: 0.0 };
    const ZERO: Self = Self { a: 0.0, b: 0.0 };

    fn constant(b: f32) -> Self {
        Self { a: 0.0, b }
    }

    fn scale(self, k: f32) -> Self {
        Self {
            a: self.a * k,
            b: self.b * k,
        }
    }

    fn plus(self, other: Self) -> Self {
        Self {
            a: self.a + other.a,
            b: self.b + other.b,
        }
    }

    fn times(self, other: Self) -> Option<Self> {
        if self.a == 0.0 {
            Some(other.scale(self.b))
        } else if other.a == 0.0 {
            Some(self.scale(other.b))
        } else {
            None
        }
    }

    fn at(self, x: f32) -> f32 {
        self.a * x + self.b
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Obj {
    Any,
    Limbs,
    Limb(usize),
    EachLimb,
}

#[derive(Debug, Clone, PartialEq)]
enum Val {
    Num(Lin),
    /// The current value of a field (by token).
    Read(u32),
    /// A field's value after `+ amount` or `× amount`.
    Rel(u32, RawChange),
    Obj(Obj),
    Func(Token),
    Unknown,
}

fn arithmetic(op: &str, left: Val, right: Val) -> Val {
    use Val::{Num, Read, Rel};
    match (op, left, right) {
        ("add", Num(l), Num(r)) => Num(l.plus(r)),
        ("sub", Num(l), Num(r)) => Num(l.plus(r.scale(-1.0))),
        ("mul", Num(l), Num(r)) => l.times(r).map_or(Val::Unknown, Num),
        ("div", Num(l), Num(r)) if r.a == 0.0 && r.b != 0.0 => Num(l.scale(1.0 / r.b)),
        ("add", Read(f), Num(n)) | ("add", Num(n), Read(f)) => Rel(f, RawChange::Add(n)),
        ("sub", Read(f), Num(n)) => Rel(f, RawChange::Add(n.scale(-1.0))),
        ("mul", Read(f), Num(n)) | ("mul", Num(n), Read(f)) => Rel(f, RawChange::Mul(n)),
        ("div", Read(f), Num(n)) if n.a == 0.0 && n.b != 0.0 => {
            Rel(f, RawChange::Mul(Lin::constant(1.0 / n.b)))
        }
        ("add", Rel(f, RawChange::Add(d)), Num(n)) => Rel(f, RawChange::Add(d.plus(n))),
        ("sub", Rel(f, RawChange::Add(d)), Num(n)) => Rel(f, RawChange::Add(d.plus(n.scale(-1.0)))),
        _ => Val::Unknown,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RawChange {
    Add(Lin),
    Mul(Lin),
    Set(Lin),
    Trigger,
}

impl RawChange {
    fn map(self, f: impl Fn(Lin) -> Lin) -> Self {
        match self {
            Self::Add(n) => Self::Add(f(n)),
            Self::Mul(n) => Self::Mul(f(n)),
            Self::Set(n) => Self::Set(f(n)),
            Self::Trigger => Self::Trigger,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Raw {
    target: Target,
    stat: String,
    change: RawChange,
    seconds: Option<Lin>,
    whole_item: bool,
    conditional: bool,
}

/// What a method does, in terms of its own float parameter.
#[derive(Debug, Default)]
struct Out {
    raw: Vec<Raw>,
    /// Change of the used item's condition.
    condition: Lin,
    contents: Vec<(ContentsUse, Lin)>,
    /// The instruction being evaluated is inside a conditional branch.
    conditional: bool,
}

impl Out {
    fn add(&mut self, target: Target, stat: &str, change: RawChange) {
        self.raw.push(Raw {
            target,
            stat: stat.to_owned(),
            change,
            seconds: None,
            whole_item: false,
            conditional: self.conditional,
        });
    }

    fn store(&mut self, owner: &str, name: &str, token: u32, object: &Val, value: Val) {
        let change = match value {
            Val::Rel(field, change) if field == token => change,
            Val::Num(n) => RawChange::Set(n),
            _ => return,
        };
        let target = match owner {
            "Body" => Target::Body,
            "Limb" => match object {
                Val::Obj(Obj::Limb(i)) => Target::Limb(*i),
                Val::Obj(Obj::EachLimb) => Target::EachLimb,
                _ => Target::TreatedLimb,
            },
            "Item" => {
                if let ("condition", RawChange::Add(n)) = (name, change) {
                    self.condition = self.condition.plus(n);
                }
                return;
            }
            // Components hold internal state; only their additive changes are meaningful.
            "CustomItemBehaviour" => return,
            _ if matches!(change, RawChange::Add(_)) => Target::Component(owner.to_owned()),
            _ => return,
        };
        self.add(target, name, change);
    }

    fn merge(&mut self, other: Out) {
        let conditional = self.conditional;
        self.raw.extend(other.raw.into_iter().map(|mut raw| {
            raw.conditional |= conditional;
            raw
        }));
        self.condition = self.condition.plus(other.condition);
        self.contents.extend(other.contents);
    }

    /// Merges a timed operation that runs `inner` once per second for `duration` seconds.
    fn merge_timed(&mut self, inner: Out, duration: Lin) {
        for mut raw in inner.raw {
            raw.conditional |= self.conditional;
            // A conditional change keeps its per-occurrence amount.
            if let (RawChange::Add(n), false) = (raw.change, raw.conditional) {
                match n.times(duration) {
                    Some(total) => raw.change = RawChange::Add(total),
                    None => continue,
                }
            }
            raw.seconds = Some(duration);
            self.raw.push(raw);
        }
    }

    /// Removes the dependency on a lambda's own float parameter: per whole
    /// item when the item's condition depends on it, otherwise at `x = 1`
    /// (a fully completed minigame).
    fn normalize(self, has_float: bool) -> Self {
        if !has_float {
            return self;
        }
        if self.condition.a == 0.0 {
            return self.at(1.0);
        }
        let per_item = 1.0 / self.condition.a.abs();
        let fix = |n: Lin| Lin::constant(n.a * per_item);
        Self {
            raw: self
                .raw
                .into_iter()
                .map(|mut r| {
                    r.change = r.change.map(fix);
                    r.seconds = r.seconds.map(fix);
                    r.whole_item = true;
                    r
                })
                .collect(),
            condition: Lin::ZERO,
            contents: self
                .contents
                .into_iter()
                .map(|(k, n)| (k, fix(n)))
                .collect(),
            conditional: false,
        }
    }

    /// Evaluates every amount at `x`.
    fn at(self, x: f32) -> Self {
        let fix = |n: Lin| Lin::constant(n.at(x));
        Self {
            raw: self
                .raw
                .into_iter()
                .map(|mut r| {
                    r.change = r.change.map(fix);
                    r.seconds = r.seconds.map(fix);
                    r
                })
                .collect(),
            condition: fix(self.condition),
            contents: self
                .contents
                .into_iter()
                .map(|(k, n)| (k, fix(n)))
                .collect(),
            conditional: false,
        }
    }

    fn effects(&self) -> Vec<Effect> {
        let list = self
            .raw
            .iter()
            .map(|r| Effect {
                target: r.target.clone(),
                stat: r.stat.clone(),
                change: match r.change {
                    RawChange::Add(n) => Change::Add(n.b),
                    RawChange::Mul(n) => Change::Multiply(n.b),
                    RawChange::Set(n) => Change::Set(n.b),
                    RawChange::Trigger => Change::Trigger,
                },
                seconds: r.seconds.map(|s| s.b).filter(|s| *s > 0.0),
                whole_item: r.whole_item,
                conditional: r.conditional,
            })
            .collect();
        merge(list)
    }
}

/// Combines additions to the same stat and drops changes that do nothing.
fn merge(effects: Vec<Effect>) -> Vec<Effect> {
    let mut out: Vec<Effect> = Vec::new();
    let mut triggers = HashSet::new();
    for effect in effects {
        if let Change::Add(v) = effect.change
            && let Some(existing) = out.iter_mut().find(|e| {
                e.target == effect.target
                    && e.stat == effect.stat
                    && e.seconds == effect.seconds
                    && e.whole_item == effect.whole_item
                    && e.conditional == effect.conditional
                    && matches!(e.change, Change::Add(_))
            })
        {
            if let Change::Add(total) = &mut existing.change {
                *total += v;
            }
            continue;
        }
        if effect.change == Change::Trigger
            && !triggers.insert((effect.target.clone(), effect.stat.clone()))
        {
            continue;
        }
        out.push(effect);
    }
    out.retain(|e| match e.change {
        Change::Add(v) => v.is_finite() && v.abs() > 1e-4,
        Change::Multiply(v) => v.is_finite() && (v - 1.0).abs() > 1e-4,
        Change::Set(v) => v.is_finite(),
        Change::Trigger => true,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_of(effect: &Effect) -> f32 {
        match effect.change {
            Change::Add(v) => v,
            other => panic!("expected an addition, got {other:?}"),
        }
    }

    #[test]
    fn linear_arithmetic_tracks_field_changes() {
        // painkillers: opiateAmount += ml * 0.01 * 140
        let per_100 = arithmetic("mul", Val::Num(Lin::X), Val::Num(Lin::constant(0.01)));
        let amount = arithmetic("mul", per_100, Val::Num(Lin::constant(140.0)));
        let changed = arithmetic("add", Val::Read(7), amount);
        let mut out = Out::default();
        out.store(
            "Painkillers",
            "opiateAmount",
            7,
            &Val::Obj(Obj::Any),
            changed,
        );
        let effects = out.at(100.0).effects();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].target, Target::Component("Painkillers".into()));
        assert!((add_of(&effects[0]) - 140.0).abs() < 1e-3);
    }

    #[test]
    fn minigame_effects_are_scaled_to_a_whole_item() {
        // bandage: num2 = x / 12; item.condition -= num2; limb.pain -= num2 * 60
        let num2 = Lin::X.scale(1.0 / 12.0);
        let mut out = Out::default();
        let any = Val::Obj(Obj::Any);
        out.store(
            "Item",
            "condition",
            1,
            &any,
            Val::Rel(1, RawChange::Add(num2.scale(-1.0))),
        );
        out.store(
            "Limb",
            "pain",
            2,
            &any,
            Val::Rel(2, RawChange::Add(num2.scale(-60.0))),
        );
        let effects = out.normalize(true).effects();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].target, Target::TreatedLimb);
        assert!(effects[0].whole_item);
        assert!((add_of(&effects[0]) + 60.0).abs() < 1e-3);
    }

    #[test]
    fn timed_operations_total_their_ticks() {
        let mut tick = Out::default();
        let step = RawChange::Add(Lin::constant(-4.5));
        tick.add(Target::TreatedLimb, "bleedAmount", step);
        let mut out = Out::default();
        out.merge_timed(tick, Lin::constant(10.0));
        let effects = out.effects();
        assert_eq!(effects[0].seconds, Some(10.0));
        assert!((add_of(&effects[0]) + 45.0).abs() < 1e-3);
    }
}
