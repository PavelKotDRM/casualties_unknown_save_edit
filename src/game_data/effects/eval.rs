//! Symbolic scalar values and bounded method evaluation.

use super::*;

#[derive(Default)]
struct EvalState {
    stores: HashMap<u16, usize>,
    locals: HashMap<u16, Val>,
    stack: Vec<Val>,
    conditional_until: u64,
}

impl EvalState {
    fn pop(&mut self) -> Val {
        self.stack.pop().unwrap_or(Val::Unknown)
    }

    fn region(&mut self, instruction: &Instruction, out: &mut Out) {
        out.conditional = instruction.rva < self.conditional_until;
        if let Some(target) = instruction
            .branch_targets
            .iter()
            .copied()
            .filter(|target| *target > instruction.rva)
            .max()
        {
            let unconditional = matches!(instruction.mnemonic, "br" | "br.s" | "leave" | "leave.s");
            if !unconditional || out.conditional {
                self.conditional_until = self.conditional_until.max(target);
            }
        }
    }

    fn load_argument(&mut self, instruction: &Instruction, signature: &SignatureMethod) -> bool {
        let Some(arg) = arg_index(instruction) else {
            return false;
        };
        let param = if signature.has_this {
            arg.checked_sub(1)
        } else {
            Some(arg)
        };
        let float = param
            .and_then(|p| signature.params.get(usize::from(p)))
            .is_some_and(|p| matches!(p.base, TypeSignature::R4));
        self.stack.push(if float {
            Val::Num(Lin::X)
        } else {
            Val::Obj(Obj::Any)
        });
        true
    }

    fn transfer_local(&mut self, instruction: &Instruction) -> bool {
        if let Some(local) = local_index(instruction, "ldloc") {
            let value = if self.stores.get(&local) == Some(&1) {
                self.locals.get(&local).cloned().unwrap_or(Val::Unknown)
            } else {
                Val::Unknown
            };
            self.stack.push(value);
        } else if let Some(local) = local_index(instruction, "stloc") {
            let value = self.pop();
            self.locals.insert(local, value);
        } else {
            return false;
        }
        true
    }

    fn calculate(&mut self, mnemonic: &str) -> bool {
        match mnemonic {
            "add" | "sub" | "mul" | "div" => {
                let right = self.pop();
                let left = self.pop();
                self.stack.push(arithmetic(mnemonic, left, right));
            }
            "rem" | "and" | "or" | "xor" | "shl" | "shr" | "shr.un" | "ceq" | "cgt" | "cgt.un"
            | "clt" | "clt.un" => {
                self.pop();
                self.pop();
                self.stack.push(Val::Unknown);
            }
            "neg" => {
                let value = self.pop();
                self.stack.push(match value {
                    Val::Num(n) => Val::Num(n.scale(-1.0)),
                    _ => Val::Unknown,
                });
            }
            m if m.starts_with("conv.") => {
                let value = self.pop();
                self.stack.push(value);
            }
            _ => return false,
        }
        true
    }

    fn branch(&mut self, mnemonic: &str) -> bool {
        match mnemonic {
            "brtrue" | "brtrue.s" | "brfalse" | "brfalse.s" | "switch" => {
                self.pop();
            }
            "br" | "br.s" | "ret" | "throw" | "leave" | "leave.s" | "endfinally" => {
                self.stack.clear();
            }
            m if ["beq", "bne", "bge", "bgt", "ble", "blt"]
                .iter()
                .any(|p| m.starts_with(p)) =>
            {
                self.pop();
                self.pop();
            }
            _ => return false,
        }
        true
    }
}

pub(super) fn is_closure(owner: &str) -> bool {
    owner.contains("DisplayClass")
}

pub(super) fn arg_index(instruction: &Instruction) -> Option<u16> {
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

pub(super) fn local_index(instruction: &Instruction, prefix: &str) -> Option<u16> {
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

pub(super) fn immediate_index(immediate: &Immediate) -> Option<u16> {
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
pub(super) struct Lin {
    pub(super) a: f32,
    pub(super) b: f32,
}

impl Lin {
    pub(super) const X: Self = Self { a: 1.0, b: 0.0 };
    pub(super) const ZERO: Self = Self { a: 0.0, b: 0.0 };

    pub(super) fn constant(b: f32) -> Self {
        Self { a: 0.0, b }
    }

    pub(super) fn scale(self, k: f32) -> Self {
        Self {
            a: self.a * k,
            b: self.b * k,
        }
    }

    pub(super) fn plus(self, other: Self) -> Self {
        Self {
            a: self.a + other.a,
            b: self.b + other.b,
        }
    }

    pub(super) fn times(self, other: Self) -> Option<Self> {
        if self.a == 0.0 {
            Some(other.scale(self.b))
        } else if other.a == 0.0 {
            Some(self.scale(other.b))
        } else {
            None
        }
    }

    pub(super) fn at(self, x: f32) -> f32 {
        self.a * x + self.b
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Obj {
    Any,
    Limbs,
    Limb(usize),
    EachLimb,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Val {
    Num(Lin),
    /// The current value of a field (by token).
    Read(u32),
    /// A field's value after `+ amount` or `× amount`.
    Rel(u32, RawChange),
    Obj(Obj),
    Func(Token),
    Unknown,
}

pub(super) fn arithmetic(op: &str, left: Val, right: Val) -> Val {
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

impl Reader<'_> {
    /// Symbolically runs one method. `closure` carries captured variables
    /// between a lambda and the lambdas it creates.
    pub(super) fn analyze(
        &self,
        token: Token,
        closure: &mut HashMap<u32, Val>,
        depth: usize,
    ) -> Out {
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
        let mut state = EvalState {
            stores,
            ..Default::default()
        };
        for instruction in &instructions {
            let mnemonic = instruction.mnemonic;
            state.region(instruction, &mut out);
            if state.load_argument(instruction, signature)
                || state.transfer_local(instruction)
                || state.calculate(mnemonic)
                || state.branch(mnemonic)
            {
                continue;
            }
            match mnemonic {
                "ldstr" | "ldnull" | "ldtoken" | "ldsfld" | "ldsflda" | "sizeof" => {
                    state.stack.push(Val::Unknown)
                }
                m if m.starts_with("ldc.") => {
                    state
                        .stack
                        .push(match instruction_value(instruction, strings) {
                            Some(StaticValue::Number(n)) => Val::Num(Lin::constant(n)),
                            _ => Val::Unknown,
                        })
                }
                m if m.starts_with("ldloca") || m.starts_with("ldarga") => {
                    state.stack.push(Val::Obj(Obj::Any))
                }
                "ldftn" => state.stack.push(match &instruction.operand {
                    Operand::Token(token) => Val::Func(*token),
                    _ => Val::Unknown,
                }),
                "ldvirtftn" => {
                    state.pop();
                    state.stack.push(Val::Unknown);
                }
                "dup" => {
                    let top = state.stack.last().cloned().unwrap_or(Val::Unknown);
                    state.stack.push(top);
                }
                "pop" | "stsfld" | "initobj" => {
                    state.pop();
                }
                "ldfld" | "ldflda" => {
                    state.pop();
                    let Operand::Token(token) = &instruction.operand else {
                        state.stack.push(Val::Unknown);
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
                    state.stack.push(value);
                }
                "stfld" => {
                    let value = state.pop();
                    let object = state.pop();
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
                    let index = state.pop();
                    let array = state.pop();
                    state.stack.push(match (array, index) {
                        (Val::Obj(Obj::Limbs), Val::Num(Lin { a, b })) if a == 0.0 && b >= 0.0 => {
                            Val::Obj(Obj::Limb(b as usize))
                        }
                        (Val::Obj(Obj::Limbs), _) => Val::Obj(Obj::EachLimb),
                        _ => Val::Unknown,
                    });
                }
                m if m.starts_with("stelem") => {
                    for _ in 0..3 {
                        state.pop();
                    }
                }
                "ldlen" | "box" | "unbox" | "unbox.any" | "castclass" | "isinst" | "newarr"
                | "not" => {
                    state.pop();
                    state.stack.push(Val::Unknown);
                }
                "call" | "callvirt" | "newobj" => {
                    self.evaluate_call(instruction, &mut state, closure, depth, &mut out);
                }
                _ => {}
            }
        }
        out
    }

    fn evaluate_call(
        &self,
        instruction: &Instruction,
        state: &mut EvalState,
        closure: &mut HashMap<u32, Val>,
        depth: usize,
        out: &mut Out,
    ) {
        let Operand::Token(token) = &instruction.operand else {
            return;
        };
        let Some(callee) = self.callee(*token) else {
            state.stack.clear();
            return;
        };
        let mut args: Vec<Val> = (0..callee.params.len()).map(|_| state.pop()).collect();
        args.reverse();
        let is_new = instruction.mnemonic == "newobj";
        let this = (callee.has_this && !is_new).then(|| state.pop());
        let call = Call {
            callee: &callee,
            is_new,
            this,
            args,
        };
        if let Some(result) = self.call(call, closure, depth, out) {
            state.stack.push(result);
        }
    }

    /// Applies a call and returns its result, if it has one.
    pub(super) fn call(
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
                // Optional C# arguments are already supplied by the caller's
                // IL. An unknown expression is not an implicit default dose.
                if let Some(amount) = num(1) {
                    out.contents.push((kind, amount));
                }
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
