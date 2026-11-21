//! Literal and item-definition readers shared by the metadata/effects walkers.

use super::ItemStats;
use dotscope::{
    CilObject,
    assembly::{Instruction, Operand},
};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone)]
pub(super) enum StaticValue {
    String(String),
    Number(f32),
}

pub(super) fn instruction_string(
    instruction: &Instruction,
    strings: Option<&dotscope::UserStrings<'_>>,
) -> Option<String> {
    let Operand::Token(token) = &instruction.operand else {
        return None;
    };
    strings?
        .get((token.value() & 0x00ff_ffff) as usize)
        .ok()
        .map(|value| value.to_string_lossy())
}

pub(super) fn instruction_value(
    instruction: &Instruction,
    strings: Option<&dotscope::UserStrings<'_>>,
) -> Option<StaticValue> {
    if instruction.mnemonic == "ldstr" {
        return instruction_string(instruction, strings).map(StaticValue::String);
    }
    if !instruction.mnemonic.starts_with("ldc.") {
        return None;
    }
    if let Operand::Immediate(value) = &instruction.operand {
        use dotscope::assembly::Immediate::*;
        let value = match value {
            Int8(value) => *value as f32,
            UInt8(value) => *value as f32,
            Int16(value) => *value as f32,
            UInt16(value) => *value as f32,
            Int32(value) => *value as f32,
            UInt32(value) => *value as f32,
            Int64(value) => *value as f32,
            UInt64(value) => *value as f32,
            Float32(value) => *value,
            Float64(value) => *value as f32,
        };
        return Some(StaticValue::Number(value));
    }
    let suffix = instruction.mnemonic.strip_prefix("ldc.i4.")?;
    let value = if suffix == "m1" {
        -1
    } else {
        suffix.parse::<i32>().ok()?
    };
    Some(StaticValue::Number(value as f32))
}

pub(super) fn number(fields: &HashMap<String, StaticValue>, key: &str) -> Option<f32> {
    match fields.get(key)? {
        StaticValue::Number(value) => Some(*value),
        _ => None,
    }
}

pub(super) fn text<'a>(fields: &'a HashMap<String, StaticValue>, key: &str) -> Option<&'a str> {
    match fields.get(key)? {
        StaticValue::String(value) => Some(value),
        _ => None,
    }
}

fn flag(fields: &HashMap<String, StaticValue>, key: &str) -> bool {
    number(fields, key).is_some_and(|value| value != 0.0)
}

pub(super) fn bool_field(fields: &HashMap<String, StaticValue>, key: &str) -> Option<bool> {
    number(fields, key).map(|value| value != 0.0)
}

pub(super) fn item_info_types(assembly: &CilObject) -> HashSet<String> {
    assembly
        .types()
        .iter()
        .filter_map(|entry| {
            let ty = entry.value();
            (ty.name == "ItemInfo" || ty.base_chain().any(|base| base.name == "ItemInfo"))
                .then(|| ty.name.clone())
        })
        .collect()
}

pub(super) fn items(assembly: &CilObject) -> Result<HashMap<String, ItemStats>, String> {
    let reader = super::metadata::Reader::new(assembly);
    let method = reader
        .method("Item", "SetupItems")
        .ok_or("Assembly-CSharp.dll has no Item.SetupItems method")?;
    let info_types = item_info_types(assembly);
    let constructors: HashMap<_, _> = assembly
        .methods()
        .iter()
        .filter_map(|entry| {
            let method = entry.value();
            let owner = method.declaring_type.get()?.upgrade()?;
            (info_types.contains(&owner.name) && method.name == ".ctor").then(|| {
                (
                    method.token.value(),
                    owner.name == "LiquidItemInfo"
                        || owner.base_chain().any(|base| base.name == "LiquidItemInfo"),
                )
            })
        })
        .collect();
    let strings = assembly.userstrings();
    let code: Vec<_> = method.instructions().collect();
    let mut current_id = None;
    let mut definitions: BTreeMap<String, HashMap<String, StaticValue>> = BTreeMap::new();
    let mut liquids = HashSet::new();
    for (index, instruction) in code.iter().enumerate() {
        let previous = index.checked_sub(1).and_then(|index| code.get(index));
        if instruction.mnemonic == "newobj"
            && let Operand::Token(token) = instruction.operand
            && let Some(liquid) = constructors.get(&token.value())
            && let Some(previous) = previous
            && previous.mnemonic == "ldstr"
            && let Some(id) = instruction_string(previous, strings)
        {
            if *liquid {
                liquids.insert(id.clone());
            }
            definitions.entry(id.clone()).or_default();
            current_id = Some(id);
        }
        if instruction.mnemonic != "stfld" {
            continue;
        }
        let Some((owner, field)) = reader.field(&instruction.operand) else {
            continue;
        };
        if !info_types.contains(&owner) {
            continue;
        }
        if let (Some(id), Some(previous)) = (&current_id, previous)
            && let Some(value) = instruction_value(previous, strings)
        {
            definitions
                .entry(id.clone())
                .or_default()
                .insert(field, value);
        }
    }
    if definitions.is_empty() {
        return Err("Item.SetupItems contained no item definitions".into());
    }
    definitions
        .into_iter()
        .map(|(id, fields)| {
            let weight = number(&fields, "weight")
                .filter(|value| value.is_finite() && *value >= 0.0)
                .ok_or_else(|| format!("Item {id:?} has no valid weight"))?;
            let category = text(&fields, "category")
                .ok_or_else(|| format!("Item {id:?} has no category"))?
                .to_owned();
            let wear_slot = text(&fields, "wearSlotId")
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
            let wearable = flag(&fields, "wearable");
            if wearable && wear_slot.is_none() {
                return Err(format!("Wearable item {id:?} has no wear slot"));
            }
            let liquid_capacity = if liquids.contains(&id) {
                Some(
                    number(&fields, "capacity")
                        .filter(|value| value.is_finite() && *value > 0.0)
                        .ok_or_else(|| format!("Liquid item {id:?} has no valid capacity"))?,
                )
            } else {
                None
            };
            Ok((
                id,
                ItemStats {
                    category,
                    weight,
                    wearable,
                    wear_slot,
                    liquid_capacity,
                    scale_weight_with_condition: flag(&fields, "scaleWeightWithCondition"),
                    armor: number(&fields, "wearableArmor"),
                    insulation: number(&fields, "wearableIsolation"),
                    item_tags: text(&fields, "tags")
                        .map(|tags| tags.split(',').map(str::to_owned).collect())
                        .unwrap_or_default(),
                    container_capacity: None,
                    container_max_item_weight: None,
                    container_encumbrance: None,
                    container_tag_restriction: None,
                    only_hold_in_hands: flag(&fields, "onlyHoldInHands"),
                    wearable_can_be_held: flag(&fields, "wearableCanBeHeld"),
                    desired_wear_limb: text(&fields, "desiredWearLimb")
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned),
                    destroy_at_zero_condition: flag(&fields, "destroyAtZeroCondition"),
                },
            ))
        })
        .collect()
}
