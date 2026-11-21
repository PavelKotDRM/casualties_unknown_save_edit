//! Small, version-tolerant readers for game-owned editor metadata.

use std::collections::{BTreeMap, HashMap};

use dotscope::CilObject;
use dotscope::assembly::{Instruction, Operand};
use dotscope::metadata::method::MethodRc;
use dotscope::metadata::signatures::TypeSignature;

use crate::catalog::ItemDef;
use crate::schema::*;

use super::program::ScalarProgram;
use super::runtime::intern;
use super::{ItemStats, StaticValue, instruction_string, instruction_value};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metadata {
    pub moodles: super::moodles::MoodleData,
    pub moodles_error: Option<String>,
    pub ammunition: Option<super::ammunition::AmmunitionData>,
    pub ammunition_error: Option<String>,
    pub items: Vec<ItemDef>,
    pub liquids: Vec<&'static str>,
    pub characters: Vec<CharacterDef>,
    pub skills: Vec<SkillDef>,
    pub layers: Vec<&'static str>,
    pub hold_slots: Vec<HoldSlotDef>,
    pub limbs: Vec<LimbDef>,
    pub endings: Vec<EndingDef>,
    pub run_settings: Vec<RunSettingDef>,
    pub presets: Vec<RunPreset>,
    pub body_groups: Vec<FieldGroup>,
    pub limb_fields: Vec<FieldSpec>,
    /// C# constructor defaults, not necessarily the initialized player prefab.
    pub body_defaults: BTreeMap<String, serde_json::Value>,
    pub limb_defaults: BTreeMap<String, serde_json::Value>,
    pub item_defaults: BTreeMap<String, serde_json::Value>,
    pub item_components: BTreeMap<String, serde_json::Value>,
    pub item_components_error: Option<String>,
    pub battery_charges: BTreeMap<String, f32>,
    pub battery_limits: BTreeMap<String, f32>,
    pub condition_uses: BTreeMap<String, Vec<super::effects::ConditionUse>>,
    pub skill_defaults: BTreeMap<String, serde_json::Value>,
    pub root_defaults: BTreeMap<String, serde_json::Value>,
    pub skill_offsets: BTreeMap<String, f32>,
    pub skill_derived: BTreeMap<String, ScalarProgram>,
    pub mood_history_length: Option<usize>,
    pub character_identity: Option<[[i64; 2]; 4]>,
    pub experience: Option<ScalarProgram>,
    pub carry_capacity: Option<ScalarProgram>,
    pub minimum_item_weight: Option<f32>,
}

pub(super) struct Reader<'a> {
    pub assembly: &'a CilObject,
    fields: HashMap<u32, (String, String)>,
}

impl<'a> Reader<'a> {
    pub fn new(assembly: &'a CilObject) -> Self {
        let fields = assembly
            .types()
            .iter()
            .flat_map(|entry| {
                let ty = entry.value();
                ty.fields()
                    .map(|field| (field.token.value(), (ty.name.clone(), field.name.clone())))
                    .collect::<Vec<_>>()
            })
            .collect();
        Self { assembly, fields }
    }

    pub fn method(&self, owner: &str, name: &str) -> Option<MethodRc> {
        self.assembly.methods().iter().find_map(|entry| {
            let method = entry.value();
            let ty = method.declaring_type.get()?.upgrade()?;
            (ty.name == owner && method.name == name).then(|| method.clone())
        })
    }

    pub fn field(&self, operand: &Operand) -> Option<(String, String)> {
        let Operand::Token(token) = operand else {
            return None;
        };
        self.fields.get(&token.value()).cloned()
    }

    pub fn callee(&self, operand: &Operand) -> Option<(String, String)> {
        let Operand::Token(token) = operand else {
            return None;
        };
        if token.table() == 0x06 {
            let entry = self.assembly.methods().get(token)?;
            let method = entry.value();
            return Some((
                method.declaring_type.get()?.upgrade()?.name.clone(),
                method.name.clone(),
            ));
        }
        let member = self.assembly.member_ref(token)?;
        Some((
            member.declaredby.name().unwrap_or_default(),
            member.name.clone(),
        ))
    }

    fn number(&self, instruction: &Instruction) -> Option<f32> {
        match instruction_value(instruction, self.assembly.userstrings())? {
            StaticValue::Number(value) => Some(value),
            _ => None,
        }
    }

    fn text(&self, instruction: &Instruction) -> Option<String> {
        (instruction.mnemonic == "ldstr")
            .then(|| instruction_string(instruction, self.assembly.userstrings()))
            .flatten()
    }
}

pub(super) fn parse(
    assembly: &CilObject,
    items: &HashMap<String, ItemStats>,
    liquids: impl Iterator<Item = String>,
    locale: Option<&crate::locale::Locale>,
) -> Metadata {
    let reader = Reader::new(assembly);
    let mut metadata = Metadata {
        items: items
            .iter()
            .map(|(id, item)| ItemDef {
                id: intern(id),
                category: intern(&item.category),
                group: "other",
                wear_slot: intern(item.wear_slot.as_deref().unwrap_or("")),
                capacity: item.liquid_capacity.unwrap_or(0.0),
            })
            .collect(),
        liquids: liquids.map(intern).collect(),
        ..Default::default()
    };
    metadata.items.sort_by_key(|item| item.id);
    metadata.liquids.sort_unstable();
    parse_settings(&reader, &mut metadata, locale);
    let (specs, values) = scalar_fields(&reader, "Body");
    metadata.body_defaults = values;
    if !specs.is_empty() {
        metadata.body_groups.push(FieldGroup {
            title: "Body",
            icon: "",
            fields: Box::leak(specs.into_boxed_slice()),
        });
    }
    let (specs, values) = scalar_fields(&reader, "Limb");
    metadata.limb_fields = specs;
    metadata.limb_defaults = values;
    metadata.item_defaults = constructor_defaults(&reader, "Item");
    let raw_item_defaults = metadata.item_defaults.clone();
    metadata
        .item_defaults
        .extend(constructor_defaults(&reader, "SavedItem"));
    if let Some(method) = reader.method("SaveSystem", "SaveGame") {
        let code: Vec<_> = method.instructions().collect();
        for window in code.windows(2) {
            if window[0].mnemonic == "ldfld"
                && window[1].mnemonic == "stfld"
                && let (Some((source, field)), Some((destination, saved_field))) = (
                    reader.field(&window[0].operand),
                    reader.field(&window[1].operand),
                )
                && source == "Item"
                && destination == "SavedItem"
                && let Some(value) = raw_item_defaults.get(&field)
            {
                metadata.item_defaults.insert(saved_field, value.clone());
            }
        }
    }
    metadata.skill_defaults = constructor_defaults(&reader, "Skills");
    metadata.root_defaults = constructor_defaults(&reader, "SaveInfo");
    let world_defaults = constructor_defaults(&reader, "WorldGeneration");
    for (saved, field) in [
        ("trapRarity", "trapRarityMultiplier"),
        ("lootRarity", "lootRarityMultiplier"),
    ] {
        if let Some(value) = world_defaults.get(field) {
            metadata.root_defaults.insert(saved.into(), value.clone());
        }
    }
    if let Some(method) = reader.method("Body", ".ctor") {
        let code: Vec<_> = method.instructions().collect();
        for window in code.windows(3) {
            if window[1].mnemonic == "newarr"
                && window[2].mnemonic == "stfld"
                && reader
                    .field(&window[2].operand)
                    .is_some_and(|(_, field)| field == "lastHappiness")
            {
                metadata.mood_history_length = reader
                    .number(window[0])
                    .map(|length| length as usize)
                    .filter(|length| *length > 0 && *length <= 4096);
            }
        }
    }
    if let Some(method) = reader.method("PlayerCamera", "Start") {
        let code: Vec<_> = method.instructions().collect();
        if let Some(end) = code.iter().position(|instruction| {
            reader
                .callee(&instruction.operand)
                .is_some_and(|(owner, method)| owner == "WoundView" && method == "SetCharDetails")
        }) {
            let mut values = Vec::new();
            for window in code[..end].windows(3) {
                if reader
                    .callee(&window[2].operand)
                    .is_some_and(|(owner, name)| owner == "Random" && name == "Range")
                    && let (Some(min), Some(max)) =
                        (reader.number(window[0]), reader.number(window[1]))
                    && min < max
                {
                    // `Random.Range(min, max)` with an exclusive maximum.
                    values.push([min as i64, max as i64]);
                }
            }
            if values.len() == 4 {
                metadata.character_identity = values.try_into().ok();
            }
        }
    }
    for entry in assembly.types().iter() {
        let ty = entry.value();
        if ty.name != "Skills" {
            continue;
        }
        for field in ty.fields().filter(|field| !field.flags.is_static()) {
            if matches!(field.signature.base, TypeSignature::I4 | TypeSignature::R4) {
                metadata
                    .skill_defaults
                    .entry(field.name.clone())
                    .or_insert_with(|| {
                        if matches!(field.signature.base, TypeSignature::I4) {
                            0.into()
                        } else {
                            serde_json::json!(0.0)
                        }
                    });
            }
        }
    }
    if let Some(method) = reader.method("Skills", "GetExperienceForLevel") {
        metadata.experience =
            ScalarProgram::read(assembly, &method.instructions().collect::<Vec<_>>());
    }
    parse_characters(&reader, &mut metadata, locale);
    for skill in &metadata.skills {
        if let Some(method) = reader.method("Skills", &format!("get_{}From10", skill.key)) {
            let code: Vec<_> = method.instructions().collect();
            if let Some(program) = ScalarProgram::read(assembly, &code) {
                metadata
                    .skill_derived
                    .insert(format!("{}From10", skill.key), program);
            }
            if code.len() == 6
                && code[4].mnemonic == "sub"
                && let Some(offset) = reader.number(code[3])
            {
                metadata.skill_offsets.insert(skill.key.into(), offset);
            }
        }
    }
    parse_endings(&reader, &mut metadata, locale);
    if let Some(locale) = locale {
        for index in 1..=1024 {
            let Some(name) = locale.other(&format!("layertitle{index}")) else {
                break;
            };
            metadata.layers.push(intern(name));
        }
    }
    if let Some(method) = reader.method("Item", "get_totalWeight") {
        let code: Vec<_> = method.instructions().collect();
        // Lerp's first argument is a literal immediately before this.weight.
        for window in code.windows(4) {
            if window[1].mnemonic == "ldarg.0"
                && reader
                    .field(&window[3].operand)
                    .is_some_and(|(_, name)| name == "weight")
            {
                metadata.minimum_item_weight = reader.number(window[0]);
            }
        }
    }
    if let Some(method) = reader.method("Body", "HandlePeriodicChecks") {
        let code: Vec<_> = method.instructions().collect();
        if let Some(end) = code.iter().position(|instruction| {
            instruction.mnemonic == "stfld"
                && reader
                    .field(&instruction.operand)
                    .is_some_and(|(_, field)| field == "maxEncumberance")
        }) {
            let start = code[..end]
                .iter()
                .rposition(|instruction| {
                    !instruction.mnemonic.starts_with("br")
                        && instruction
                            .branch_targets
                            .iter()
                            .any(|target| *target > code[end].rva)
                })
                .map(|i| i + 1);
            if let Some(start) = start {
                metadata.carry_capacity = ScalarProgram::read(assembly, &code[start..=end]);
            }
        }
    }
    metadata
}

pub(super) fn refresh_targets(metadata: &mut Metadata) {
    let update = |fields: &mut [FieldSpec], defaults: &BTreeMap<String, serde_json::Value>| {
        for field in fields {
            if field.kind == Kind::Gauge
                && let Some(value) = defaults.get(field.key).and_then(serde_json::Value::as_f64)
                && value >= f64::from(field.min)
                && value <= f64::from(field.max)
            {
                field.good = Good::Near(value as f32);
            }
        }
    };
    for group in &mut metadata.body_groups {
        let mut fields = group.fields.to_vec();
        update(&mut fields, &metadata.body_defaults);
        group.fields = Box::leak(fields.into_boxed_slice());
    }
    update(&mut metadata.limb_fields, &metadata.limb_defaults);
}

pub(super) fn apply_awake_defaults(assembly: &CilObject, metadata: &mut Metadata) {
    let reader = Reader::new(assembly);
    for (owner, defaults) in [
        ("Body", &mut metadata.body_defaults),
        ("Limb", &mut metadata.limb_defaults),
    ] {
        let Some(method) = reader.method(owner, "Awake") else {
            continue;
        };
        let code: Vec<_> = method.instructions().collect();
        for window in code.windows(3) {
            if window[0].mnemonic == "ldarg.0"
                && window[2].mnemonic == "stfld"
                && let (Some(value), Some((ty, name))) =
                    (reader.number(window[1]), reader.field(&window[2].operand))
                && ty == owner
                && defaults.contains_key(&name)
            {
                let previous = &defaults[&name];
                defaults.insert(
                    name,
                    if previous.is_boolean() {
                        (value != 0.0).into()
                    } else {
                        serde_json::json!(value)
                    },
                );
            }
        }
    }
}

pub(super) fn constructor_defaults(
    reader: &Reader<'_>,
    owner: &str,
) -> BTreeMap<String, serde_json::Value> {
    let mut defaults = BTreeMap::new();
    let method = reader.method(owner, ".ctor");
    let code: Vec<_> = method
        .as_ref()
        .map(|method| method.instructions().collect())
        .unwrap_or_default();
    for window in code.windows(2) {
        if window[1].mnemonic != "stfld" {
            continue;
        }
        let Some((ty, field)) = reader.field(&window[1].operand) else {
            continue;
        };
        if ty != owner {
            continue;
        }
        if let Some(value) = instruction_value(window[0], reader.assembly.userstrings()) {
            defaults.insert(
                field,
                match value {
                    StaticValue::Number(value) => serde_json::json!(value),
                    StaticValue::String(value) => value.into(),
                },
            );
        }
    }
    // Fields the constructor leaves alone still serialize with C#'s zero value.
    for entry in reader.assembly.types().iter() {
        let ty = entry.value();
        if ty.name != owner {
            continue;
        }
        for field in ty.fields().filter(|field| !field.flags.is_static()) {
            let zero = match field.signature.base {
                TypeSignature::I4 => serde_json::json!(0),
                TypeSignature::R4 | TypeSignature::R8 => serde_json::json!(0.0),
                TypeSignature::Boolean => serde_json::json!(false),
                TypeSignature::String => serde_json::Value::Null,
                _ => continue,
            };
            let value = defaults.entry(field.name.clone()).or_insert(zero);
            if matches!(field.signature.base, TypeSignature::Boolean) && !value.is_boolean() {
                *value = serde_json::json!(value.as_f64().is_some_and(|value| value != 0.0));
            }
        }
    }
    defaults
}

fn scalar_fields(
    reader: &Reader<'_>,
    owner: &str,
) -> (Vec<FieldSpec>, BTreeMap<String, serde_json::Value>) {
    let mut specs = Vec::new();
    let mut defaults = constructor_defaults(reader, owner);
    for entry in reader.assembly.types().iter() {
        let ty = entry.value();
        if ty.name != owner {
            continue;
        }
        for field in ty.fields() {
            let serialized = field
                .custom_attributes
                .iter()
                .any(|(_, attribute)| match &attribute.constructor {
                    dotscope::metadata::typesystem::CilTypeReference::MemberRef(member) => member
                        .declaredby
                        .name()
                        .is_some_and(|name| name == "JsonPropertyAttribute"),
                    _ => false,
                });
            if !serialized || field.flags.is_static() {
                continue;
            }
            let (kind, zero) = match &field.signature.base {
                TypeSignature::Boolean => (Kind::Flag, serde_json::json!(false)),
                TypeSignature::I4 => (Kind::Int, serde_json::json!(0)),
                TypeSignature::R4 | TypeSignature::R8 => (Kind::Number, serde_json::json!(0.0)),
                _ => continue,
            };
            let default = defaults.entry(field.name.clone()).or_insert(zero);
            if kind == Kind::Flag {
                *default = serde_json::json!(
                    default.as_f64().is_some_and(|value| value != 0.0)
                        || default.as_bool() == Some(true)
                );
            }
            specs.push(FieldSpec {
                key: intern(&field.name),
                label: intern(&field.name),
                help: "",
                kind,
                min: f32::NEG_INFINITY,
                max: f32::INFINITY,
                unit: "",
                good: Good::Neutral,
            });
        }
    }
    for entry in reader.assembly.methods().iter() {
        let method = entry.value();
        if !method
            .declaring_type
            .get()
            .and_then(|ty| ty.upgrade())
            .is_some_and(|ty| ty.name == owner)
        {
            continue;
        }
        let code: Vec<_> = method.instructions().collect();
        for window in code.windows(4) {
            if window[3].mnemonic != "stfld"
                || !reader
                    .callee(&window[2].operand)
                    .is_some_and(|(owner, method)| owner == "Mathf" && method == "Clamp")
            {
                continue;
            }
            if let (Some(min), Some(max), Some((_, name))) = (
                reader.number(window[0]),
                reader.number(window[1]),
                reader.field(&window[3].operand),
            ) && min.is_finite()
                && max.is_finite()
                && min < max
                && let Some(spec) = specs
                    .iter_mut()
                    .find(|spec| spec.key == name && spec.kind == Kind::Number)
            {
                spec.min = min;
                spec.max = max;
                spec.kind = Kind::Gauge;
            }
        }
    }
    (specs, defaults)
}

fn parse_settings(
    reader: &Reader<'_>,
    metadata: &mut Metadata,
    locale: Option<&crate::locale::Locale>,
) {
    let Some(method) = reader.method("RunSettings", ".cctor") else {
        return;
    };
    let code: Vec<_> = method.instructions().collect();
    let mut segment = 0;
    let mut section = "";
    let mut preset: Option<(&'static str, Vec<(&'static str, PresetValue)>)> = None;
    for (index, instruction) in code.iter().enumerate() {
        let callee = reader.callee(&instruction.operand);
        if instruction.mnemonic == "newobj"
            && let Some((owner, _)) = &callee
            && matches!(
                owner.as_str(),
                "RunSettingBool" | "RunSettingFloat" | "RunSettingDropdown"
            )
        {
            let strings: Vec<_> = code[segment..index]
                .iter()
                .filter_map(|instruction| reader.text(instruction))
                .collect();
            let Some(id) = strings.first() else { continue };
            let kind = match owner.as_str() {
                "RunSettingBool" => RunSettingKind::Bool,
                "RunSettingFloat" => RunSettingKind::Float {
                    min: f32::NEG_INFINITY,
                    max: f32::INFINITY,
                    whole: false,
                },
                _ => RunSettingKind::Dropdown(Box::leak(
                    strings[1..]
                        .iter()
                        .map(intern)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )),
            };
            metadata.run_settings.push(RunSettingDef {
                id: intern(id),
                label: intern(
                    locale
                        .and_then(|locale| locale.other(&format!("runset{id}")))
                        .unwrap_or(id),
                ),
                kind,
                postfix: "",
                section,
            });
        } else if instruction.mnemonic == "stfld"
            && index > 0
            && let Some((_, name)) = reader.field(&instruction.operand)
        {
            if name == "presetName" {
                if let Some((id, values)) = preset.take() {
                    metadata.presets.push(RunPreset {
                        id,
                        name: id,
                        overrides: Box::leak(values.into_boxed_slice()),
                    });
                }
                preset = reader
                    .text(code[index - 1])
                    .map(|id| (intern(id), Vec::new()));
            } else if let Some(setting) = metadata
                .run_settings
                .last_mut()
                .filter(|_| preset.is_none())
            {
                match name.as_str() {
                    "divider" => {
                        section = reader.text(code[index - 1]).map(intern).unwrap_or("");
                        setting.section = section;
                    }
                    "postfix" => {
                        setting.postfix = reader.text(code[index - 1]).map(intern).unwrap_or("")
                    }
                    "wholeNum" => {
                        if let RunSettingKind::Float { whole, .. } = &mut setting.kind {
                            *whole = reader.number(code[index - 1]) == Some(1.0);
                        }
                    }
                    "limits" if index >= 3 => {
                        if let (
                            Some(min),
                            Some(max),
                            RunSettingKind::Float {
                                min: lo, max: hi, ..
                            },
                        ) = (
                            reader.number(code[index - 3]),
                            reader.number(code[index - 2]),
                            &mut setting.kind,
                        ) {
                            *lo = min;
                            *hi = max;
                        }
                    }
                    _ => {}
                }
            }
        } else if instruction.mnemonic == "callvirt"
            && callee.as_ref().is_some_and(|(_, name)| name == "Add")
        {
            if let Some((_, values)) = &mut preset {
                // Dictionary.Add(string, boxed primitive).
                if index >= 3
                    && code[index - 1].mnemonic == "box"
                    && let (Some(id), Some(value)) =
                        (reader.text(code[index - 3]), reader.number(code[index - 2]))
                    && let Some(setting) = metadata
                        .run_settings
                        .iter()
                        .find(|setting| setting.id == id)
                {
                    values.push((
                        intern(id),
                        match setting.kind {
                            RunSettingKind::Bool => PresetValue::Bool(value != 0.0),
                            RunSettingKind::Dropdown(_) => PresetValue::Index(value as i64),
                            _ => PresetValue::Float(value),
                        },
                    ));
                }
            }
            segment = index + 1;
        }
    }
    if let Some((id, values)) = preset {
        metadata.presets.push(RunPreset {
            id,
            name: id,
            overrides: Box::leak(values.into_boxed_slice()),
        });
    }
}

fn parse_characters(
    reader: &Reader<'_>,
    metadata: &mut Metadata,
    locale: Option<&crate::locale::Locale>,
) {
    let arrays = |name, floats| -> Vec<[f32; 3]> {
        let Some(method) = reader.method("Skills", name) else {
            return Vec::new();
        };
        let code: Vec<_> = method.instructions().collect();
        let count = code
            .iter()
            .find_map(|instruction| match &instruction.operand {
                Operand::Switch(targets) => Some(targets.len()),
                _ => None,
            })
            .unwrap_or(0);
        let mut arrays = Vec::new();
        let mut current = [0.0; 3];
        let mut started = false;
        for (index, instruction) in code.iter().enumerate() {
            if instruction.mnemonic == "newarr" {
                current = [0.0; 3];
                started = index > 0 && reader.number(code[index - 1]) == Some(3.0);
            } else if instruction.mnemonic == "ldtoken"
                && started
                && let Operand::Token(token) = instruction.operand
            {
                for entry in reader.assembly.types().iter() {
                    for field in entry.value().fields() {
                        if field.token != token {
                            continue;
                        }
                        let Some(rva) = field.rva.get() else { continue };
                        let file = reader.assembly.file();
                        let Some(bytes) = file
                            .rva_to_offset(*rva as usize)
                            .ok()
                            .and_then(|offset| file.data_slice(offset, 12).ok())
                        else {
                            continue;
                        };
                        for (slot, &bytes) in current.iter_mut().zip(bytes.as_chunks::<4>().0) {
                            *slot = if floats {
                                f32::from_le_bytes(bytes)
                            } else {
                                i32::from_le_bytes(bytes) as f32
                            };
                        }
                    }
                }
            } else if instruction.mnemonic.starts_with("stelem.") && started && index >= 2 {
                if let (Some(slot), Some(value)) = (
                    reader.number(code[index - 2]),
                    reader.number(code[index - 1]),
                ) && let Some(destination) = current.get_mut(slot as usize)
                {
                    *destination = value;
                }
            } else if instruction.mnemonic == "ret" && started && arrays.len() < count {
                arrays.push(current);
                started = false;
            }
        }
        arrays
    };
    let base = arrays("BaseSkills", false);
    let multipliers = arrays("BaseMultipliers", true);
    for (index, (base, multipliers)) in base.iter().zip(&multipliers).enumerate() {
        let key = format!("char{index}name");
        metadata.characters.push(CharacterDef {
            name: intern(locale.and_then(|locale| locale.other(&key)).unwrap_or(&key)),
            base_skills: base.map(|value| value as i64),
            gain_multipliers: multipliers.map(f64::from),
            height_multiplier: None,
        });
    }
    if let Some(method) = reader.method("Skills", "Setup") {
        let code: Vec<_> = method.instructions().collect();
        for instruction in code {
            if instruction.mnemonic == "stfld"
                && let Some((owner, name)) = reader.field(&instruction.operand)
                && owner == "Skills"
                && !name.ends_with("GainMult")
                && !name.starts_with("exp")
            {
                metadata.skills.push(SkillDef {
                    key: intern(&name),
                    name: intern(&name),
                    locale_key: "",
                    help: "",
                });
            }
            if let Some(method) = reader.method("Skills", "AddExp") {
                let code: Vec<_> = method.instructions().collect();
                let keys: Vec<_> = code
                    .windows(2)
                    .filter_map(|window| {
                        reader
                            .callee(&window[1].operand)
                            .filter(|(owner, method)| owner == "Locale" && method == "GetOther")?;
                        reader.text(window[0])
                    })
                    .collect();
                let repeated = keys
                    .iter()
                    .max_by_key(|key| keys.iter().filter(|other| other == key).count());
                let labels: Vec<_> = keys.iter().filter(|key| Some(*key) != repeated).collect();
                if labels.len() == metadata.skills.len() {
                    for (skill, key) in metadata.skills.iter_mut().zip(labels) {
                        skill.locale_key = intern(key);
                        skill.name =
                            intern(locale.and_then(|locale| locale.other(key)).unwrap_or(key));
                    }
                }
            }
        }
    }
}

fn parse_endings(
    reader: &Reader<'_>,
    metadata: &mut Metadata,
    locale: Option<&crate::locale::Locale>,
) {
    let mut endings = BTreeMap::new();
    for entry in reader.assembly.methods().iter() {
        let method = entry.value();
        let code: Vec<_> = method.instructions().collect();
        let mut pref = None;
        let mut achievement = None;
        let mut ending = None;
        for (index, instruction) in code.iter().enumerate() {
            let Some((owner, name)) = reader.callee(&instruction.operand) else {
                continue;
            };
            match (owner.as_str(), name.as_str()) {
                ("PlayerPrefs", "SetInt") if index >= 2 => {
                    if let Some(key) = reader.text(code[index - 2]) {
                        pref = Some(key);
                    }
                }
                ("SteamManager", "TryUnlockAchievement") if index > 0 => {
                    if achievement.is_none() {
                        achievement = code[index.saturating_sub(3)..index]
                            .iter()
                            .rev()
                            .find_map(|instruction| reader.text(instruction));
                    }
                }
                ("PlayerCamera", "EndSequence") if index > 0 => {
                    ending = reader.number(code[index - 1]).map(|value| value as usize);
                }
                _ => {}
            }
        }
        if let (Some(index), Some(pref), Some(achievement)) = (ending, pref, achievement) {
            endings.entry(index).or_insert((pref, achievement));
        }
    }
    let status_method = reader.assembly.methods().iter().find_map(|entry| {
        let method = entry.value();
        let owner = method.declaring_type.get()?.upgrade()?;
        (owner.name.contains("EndSequence") && method.name == "MoveNext").then(|| method.clone())
    });
    let mut statuses = BTreeMap::new();
    if let Some(method) = status_method {
        let code: Vec<_> = method.instructions().collect();
        for (index, instruction) in code.iter().enumerate() {
            if instruction.mnemonic.starts_with("bne.un")
                && index >= 2
                && reader
                    .field(&code[index - 2].operand)
                    .is_some_and(|(_, name)| name == "type")
                && let Some(number) = reader.number(code[index - 1])
                && let Some(key) = code[index + 1..]
                    .iter()
                    .find_map(|instruction| reader.text(instruction))
            {
                statuses.insert(number as usize, key);
            }
            if instruction.mnemonic != "switch"
                || index == 0
                || !reader
                    .field(&code[index - 1].operand)
                    .is_some_and(|(_, name)| name == "type")
            {
                continue;
            }
            for (index, target) in instruction.branch_targets.iter().enumerate() {
                if let Some(start) = code
                    .iter()
                    .position(|instruction| instruction.rva == *target)
                    && let Some(key) = code[start..]
                        .iter()
                        .find_map(|instruction| reader.text(instruction))
                {
                    statuses.insert(index, key);
                }
            }
        }
    }
    for (index, (pref, achievement)) in endings {
        let status_key = statuses.get(&index).map(String::as_str).unwrap_or("");
        metadata.endings.push(EndingDef {
            pref_key: intern(&pref),
            achievement: intern(&achievement),
            name: intern(&achievement),
            status_key: intern(status_key),
            status: intern(
                locale
                    .and_then(|locale| locale.other(status_key))
                    .unwrap_or(status_key),
            ),
            how: "",
        });
    }
}
