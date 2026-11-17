//! Definition and delegate metadata readers.

use super::*;

/// One `new ItemInfo { … }` / `new LiquidType { … }` initialiser.
#[derive(Default)]
pub(super) struct Definition {
    pub(super) id: String,
    pub(super) fields: HashMap<String, StaticValue>,
    pub(super) lambdas: HashMap<String, Token>,
    pub(super) qualities: Vec<(String, f32)>,
    pub(super) contents: Vec<(String, f32)>,
    pub(super) recognition: Option<i32>,
}

pub(super) struct Callee {
    pub(super) owner: String,
    pub(super) name: String,
    /// Whether each parameter is a `float`.
    pub(super) params: Vec<bool>,
    pub(super) has_this: bool,
    pub(super) returns: bool,
}

pub(super) struct Reader<'a> {
    pub(super) assembly: &'a CilObject,
    pub(super) fields: HashMap<u32, (String, String)>,
    liquid_injection_sickness: Option<f32>,
}

impl<'a> Reader<'a> {
    pub(super) fn new(assembly: &'a CilObject) -> Self {
        let mut fields = HashMap::new();
        for entry in assembly.types().iter() {
            let ty = entry.value();
            for field in ty.fields() {
                fields.insert(field.token.value(), (ty.name.clone(), field.name.clone()));
            }
        }
        let metadata_reader = super::super::metadata::Reader::new(assembly);
        let defaults = super::super::metadata::constructor_defaults(&metadata_reader, "LiquidType");
        let liquid_injection_sickness = defaults
            .get("injectionSickness")
            .and_then(serde_json::Value::as_f64)
            .map(|value| value as f32);
        Self {
            assembly,
            fields,
            liquid_injection_sickness,
        }
    }

    pub(super) fn find_method(&self, owner: &str, name: &str) -> Option<MethodRc> {
        self.assembly.methods().iter().find_map(|entry| {
            let method = entry.value();
            let ty = method.declaring_type.get()?.upgrade()?;
            (ty.name == owner && method.name == name).then(|| method.clone())
        })
    }

    pub(super) fn callee(&self, token: Token) -> Option<Callee> {
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

    pub(super) fn field(&self, operand: &Operand) -> Option<&(String, String)> {
        let Operand::Token(token) = operand else {
            return None;
        };
        self.fields.get(&token.value())
    }

    /// Splits a definition method into the initialisers of `owners` types,
    /// keyed by the string just before `newobj` (`Add("id", new …)`).
    pub(super) fn definitions(&self, method: &MethodRc, owners: &[&str]) -> Vec<Definition> {
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
                            if let (Some(id), Some(level)) = (text(0), number(1)) {
                                def.qualities.push((id, level));
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

    pub(super) fn has_float_param(&self, token: Token) -> bool {
        self.callee(token).is_some_and(|c| c.params.contains(&true))
    }

    pub(super) fn item_profile(&self, def: &Definition) -> ItemProfile {
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

    pub(super) fn liquid_profile(&self, def: &Definition) -> LiquidProfile {
        let per_100_ml = |field: &str| {
            def.lambdas.get(field).map_or_else(Vec::new, |token| {
                let mut closure = HashMap::new();
                self.analyze(*token, &mut closure, 0)
                    .at(LIQUID_EFFECT_VOLUME_ML)
                    .effects()
            })
        };
        LiquidProfile {
            locale_name: super::string_field(&def.fields, "localeName").map(str::to_owned),
            value_per_liter: super::number_field(&def.fields, "valuePerLiter"),
            injectable: super::bool_field(&def.fields, "injectable").unwrap_or(false),
            injection_sickness: super::number_field(&def.fields, "injectionSickness")
                .or(self.liquid_injection_sickness),
            qualities: def.qualities.clone(),
            drink: per_100_ml("onDrink"),
            inject: per_100_ml("onHealthUse"),
        }
    }
}

pub(super) struct Call<'a> {
    pub(super) callee: &'a Callee,
    pub(super) is_new: bool,
    pub(super) this: Option<Val>,
    pub(super) args: Vec<Val>,
}
