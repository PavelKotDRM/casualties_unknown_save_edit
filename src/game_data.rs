//! Item metadata extracted from the installed game without executing its code.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use dotscope::assembly::Operand;
use dotscope::{CilObject, ValidationConfig};
use unity_rs_core::studio::Studio;

/// Verified item fields read from `Assembly-CSharp.dll` and Unity assets.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemStats {
    pub category: String,
    pub weight: f32,
    pub scale_weight_with_condition: bool,
    pub wearable: bool,
    pub wear_slot: Option<String>,
    pub armor: Option<f32>,
    pub insulation: Option<f32>,
    pub container_encumbrance: Option<f32>,
}

impl ItemStats {
    /// Weight after applying the game's condition scaling, when enabled.
    pub fn total_weight(&self, condition: f32) -> f32 {
        if self.scale_weight_with_condition {
            self.weight * (0.1 + 0.9 * condition.clamp(0.0, 1.0))
        } else {
            self.weight
        }
    }
}

#[derive(Debug, Clone)]
enum StaticValue {
    String(String),
    Number(f32),
}

/// Reads item definitions from the installed game directory.
pub fn load_item_stats(game_dir: &Path) -> Result<HashMap<String, ItemStats>, String> {
    let data_dir = game_dir.join("CasualtiesUnknown_Data");
    let assembly_path = data_dir.join("Managed").join("Assembly-CSharp.dll");
    let assembly =
        CilObject::from_path_with_validation(&assembly_path, ValidationConfig::minimal())
            .map_err(|error| format!("Cannot parse {}: {error}", assembly_path.display()))?;
    let mut stats = parse_item_definitions(&assembly)?;
    let multipliers = parse_container_multipliers(&data_dir)?;
    for (id, multiplier) in multipliers {
        if let Some(item) = stats.get_mut(&id) {
            item.container_encumbrance = Some(multiplier);
        }
    }
    Ok(stats)
}

fn parse_item_definitions(assembly: &CilObject) -> Result<HashMap<String, ItemStats>, String> {
    let strings = assembly.userstrings();
    let mut field_names = HashMap::new();
    for entry in assembly.types().iter() {
        let ty = entry.value();
        for field in ty.fields() {
            field_names.insert(field.token.value(), (ty.name.clone(), field.name.clone()));
        }
    }

    let constructors: HashSet<_> = assembly
        .methods()
        .iter()
        .filter_map(|entry| {
            let method = entry.value();
            let owner = method
                .declaring_type
                .get()
                .and_then(|reference| reference.upgrade())?;
            (owner.name == "ItemInfo" && method.name == ".ctor").then_some(method.token.value())
        })
        .collect();

    let mut parsed_items = None;
    for entry in assembly.methods().iter() {
        let method = entry.value();
        let owner = method
            .declaring_type
            .get()
            .and_then(|reference| reference.upgrade());
        if !owner.is_some_and(|ty| ty.name == "Item") || method.name != "SetupItems" {
            continue;
        }
        let instructions: Vec<_> = method.instructions().collect();
        let mut current_id: Option<String> = None;
        let mut items: BTreeMap<String, HashMap<String, StaticValue>> = BTreeMap::new();

        for (index, instruction) in instructions.iter().enumerate() {
            if instruction.mnemonic == "newobj"
                && let Operand::Token(token) = &instruction.operand
                && constructors.contains(&token.value())
                && let Some(previous) = index.checked_sub(1).and_then(|i| instructions.get(i))
                && previous.mnemonic == "ldstr"
                && let Some(id) = instruction_string(previous, strings)
            {
                current_id = Some(id.clone());
                items.entry(id).or_default();
            }

            if instruction.mnemonic != "stfld" {
                continue;
            }
            let Operand::Token(token) = &instruction.operand else {
                continue;
            };
            let Some((owner, field)) = field_names.get(&token.value()) else {
                continue;
            };
            if owner != "ItemInfo" {
                continue;
            }
            let (Some(id), Some(previous)) = (
                current_id.as_ref(),
                index.checked_sub(1).and_then(|i| instructions.get(i)),
            ) else {
                continue;
            };
            if let Some(value) = instruction_value(previous, strings) {
                items
                    .entry(id.clone())
                    .or_default()
                    .insert(field.clone(), value);
            }
        }
        parsed_items = Some(items);
        break;
    }
    let items = parsed_items.ok_or("Assembly-CSharp.dll has no Item.SetupItems method")?;

    if items.is_empty() {
        return Err("Item.SetupItems contained no item definitions".to_owned());
    }
    items
        .into_iter()
        .map(|(id, fields)| {
            let weight = number_field(&fields, "weight")
                .filter(|value| value.is_finite() && *value >= 0.0)
                .ok_or_else(|| format!("Item {id:?} has no valid weight"))?;
            let category = string_field(&fields, "category")
                .ok_or_else(|| format!("Item {id:?} has no category"))?
                .to_owned();
            let wearable = bool_field(&fields, "wearable").unwrap_or(false);
            let wear_slot = string_field(&fields, "wearSlotId")
                .filter(|slot| !slot.is_empty())
                .map(str::to_owned);
            if wearable && wear_slot.is_none() {
                return Err(format!("Wearable item {id:?} has no wear slot"));
            }
            Ok((
                id,
                ItemStats {
                    category,
                    weight,
                    scale_weight_with_condition: bool_field(&fields, "scaleWeightWithCondition")
                        .unwrap_or(false),
                    wearable,
                    wear_slot,
                    armor: number_field(&fields, "wearableArmor"),
                    insulation: number_field(&fields, "wearableIsolation"),
                    container_encumbrance: None,
                },
            ))
        })
        .collect()
}

fn instruction_string(
    instruction: &dotscope::assembly::Instruction,
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

fn instruction_value(
    instruction: &dotscope::assembly::Instruction,
    strings: Option<&dotscope::UserStrings<'_>>,
) -> Option<StaticValue> {
    if instruction.mnemonic == "ldstr" {
        return instruction_string(instruction, strings).map(StaticValue::String);
    }
    if let Operand::Immediate(value) = &instruction.operand {
        let number = match value {
            dotscope::assembly::Immediate::Int8(value) => *value as f32,
            dotscope::assembly::Immediate::UInt8(value) => *value as f32,
            dotscope::assembly::Immediate::Int16(value) => *value as f32,
            dotscope::assembly::Immediate::UInt16(value) => *value as f32,
            dotscope::assembly::Immediate::Int32(value) => *value as f32,
            dotscope::assembly::Immediate::UInt32(value) => *value as f32,
            dotscope::assembly::Immediate::Int64(value) => *value as f32,
            dotscope::assembly::Immediate::UInt64(value) => *value as f32,
            dotscope::assembly::Immediate::Float32(value) => *value,
            dotscope::assembly::Immediate::Float64(value) => *value as f32,
        };
        return Some(StaticValue::Number(number));
    }
    let value = match instruction.mnemonic {
        "ldc.i4.m1" => -1,
        "ldc.i4.0" => 0,
        "ldc.i4.1" => 1,
        "ldc.i4.2" => 2,
        "ldc.i4.3" => 3,
        "ldc.i4.4" => 4,
        "ldc.i4.5" => 5,
        "ldc.i4.6" => 6,
        "ldc.i4.7" => 7,
        "ldc.i4.8" => 8,
        _ => return None,
    };
    Some(StaticValue::Number(value as f32))
}

fn number_field(fields: &HashMap<String, StaticValue>, key: &str) -> Option<f32> {
    match fields.get(key)? {
        StaticValue::Number(value) => Some(*value),
        _ => None,
    }
}

fn string_field<'a>(fields: &'a HashMap<String, StaticValue>, key: &str) -> Option<&'a str> {
    match fields.get(key)? {
        StaticValue::String(value) => Some(value),
        _ => None,
    }
}

fn bool_field(fields: &HashMap<String, StaticValue>, key: &str) -> Option<bool> {
    match fields.get(key)? {
        StaticValue::Number(value) => Some(*value != 0.0),
        _ => None,
    }
}

fn parse_container_multipliers(data_dir: &Path) -> Result<HashMap<String, f32>, String> {
    let studio = Studio::open(data_dir).map_err(|error| {
        format!(
            "Cannot read Unity assets in {}: {error}",
            data_dir.display()
        )
    })?;
    let version = studio
        .files()
        .next()
        .map(|file| file.unity_version().to_owned())
        .ok_or("Unity data directory contains no serialized files")?;
    if !version.starts_with("2022.") {
        return Err(format!(
            "Unsupported Unity serialization version {version:?}"
        ));
    }

    let mut multipliers = HashMap::new();
    for container in studio
        .objects()
        .filter(|object| object.class_id() == 114 && object.name() == Some("Container"))
    {
        let bytes = container
            .read_raw(4096)
            .map_err(|error| format!("Cannot read a Container component: {error}"))?;
        let game_object = read_pptr(&bytes, 0)
            .ok_or_else(|| "Container component has a truncated GameObject reference".to_owned())?;
        if game_object.0 != 0 {
            continue;
        }
        let data_offset = mono_behaviour_data_offset(&bytes)
            .ok_or_else(|| "Container component has an invalid MonoBehaviour header".to_owned())?;
        let multiplier = read_f32(&bytes, data_offset + 8)
            .filter(|value| value.is_finite() && *value >= 0.0)
            .ok_or_else(|| "Container has an invalid encumbrance multiplier".to_owned())?;
        let game_object = studio
            .object(container.file_index(), game_object.1)
            .ok_or_else(|| "Container refers to a missing GameObject".to_owned())?;
        let object_bytes = game_object
            .read_raw(1024 * 1024)
            .map_err(|error| format!("Cannot read a container GameObject: {error}"))?;
        let count = read_i32(&object_bytes, 0)
            .filter(|count| (0..=1024).contains(count))
            .ok_or_else(|| "Container GameObject has an invalid component count".to_owned())?
            as usize;
        let refs_end = 4usize
            .checked_add(count.checked_mul(12).ok_or("Component count overflow")?)
            .ok_or("Component array size overflow")?;
        if refs_end > object_bytes.len() {
            return Err("Container GameObject has a truncated component list".to_owned());
        }
        for index in 0..count {
            let Some((file_id, path_id)) = read_pptr(&object_bytes, 4 + index * 12) else {
                return Err("Container GameObject has an invalid component reference".to_owned());
            };
            if file_id != 0 {
                continue;
            }
            let Some(item) = studio.object(container.file_index(), path_id) else {
                continue;
            };
            if item.class_id() != 114 || item.name() != Some("Item") {
                continue;
            }
            let item_bytes = item
                .read_raw(1024 * 1024)
                .map_err(|error| format!("Cannot read an Item component: {error}"))?;
            let item_offset = mono_behaviour_data_offset(&item_bytes)
                .ok_or_else(|| "Item component has an invalid MonoBehaviour header".to_owned())?;
            let id = read_aligned_string(&item_bytes, item_offset)
                .ok_or_else(|| "Item component has an invalid item id".to_owned())?;
            if let Some(previous) = multipliers.insert(id.clone(), multiplier)
                && (previous - multiplier).abs() > f32::EPSILON
            {
                return Err(format!(
                    "Container metadata for item {id:?} has conflicting multipliers"
                ));
            }
        }
    }
    Ok(multipliers)
}

fn mono_behaviour_data_offset(bytes: &[u8]) -> Option<usize> {
    let mut offset = 12usize;
    offset = offset.checked_add(1)?;
    offset = offset.checked_add(3)?;
    offset = offset.checked_add(12)?;
    let (_, next) = read_aligned_string_at(bytes, offset)?;
    Some(next)
}

fn read_aligned_string(bytes: &[u8], offset: usize) -> Option<String> {
    read_aligned_string_at(bytes, offset).map(|(value, _)| value)
}

fn read_aligned_string_at(bytes: &[u8], offset: usize) -> Option<(String, usize)> {
    let length = usize::try_from(read_i32(bytes, offset)?).ok()?;
    if length > 16 * 1024 * 1024 {
        return None;
    }
    let start = offset.checked_add(4)?;
    let end = start.checked_add(length)?;
    let value = std::str::from_utf8(bytes.get(start..end)?).ok()?.to_owned();
    let next = end.checked_add(3)? & !3;
    Some((value, next))
}

fn read_pptr(bytes: &[u8], offset: usize) -> Option<(i32, i64)> {
    Some((
        read_i32(bytes, offset)?,
        i64::from_le_bytes(bytes.get(offset + 4..offset + 12)?.try_into().ok()?),
    ))
}

fn read_i32(bytes: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_f32(bytes: &[u8], offset: usize) -> Option<f32> {
    Some(f32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_stats(scale_weight_with_condition: bool) -> ItemStats {
        ItemStats {
            category: "tool".to_owned(),
            weight: 2.0,
            scale_weight_with_condition,
            wearable: false,
            wear_slot: None,
            armor: None,
            insulation: None,
            container_encumbrance: None,
        }
    }

    #[test]
    fn item_weight_applies_condition_scaling_only_when_defined() {
        let scaled = item_stats(true);
        assert!((scaled.total_weight(0.0) - 0.2).abs() < f32::EPSILON);
        assert!((scaled.total_weight(0.5) - 1.1).abs() < f32::EPSILON);
        assert_eq!(scaled.total_weight(2.0), 2.0);
        assert_eq!(item_stats(false).total_weight(0.0), 2.0);
    }

    #[test]
    fn mono_behaviour_header_reads_aligned_name_and_rejects_truncation() {
        let mut bytes = vec![0; 28];
        bytes.extend_from_slice(&3_i32.to_le_bytes());
        bytes.extend_from_slice(b"Bag");
        bytes.push(0);

        assert_eq!(read_aligned_string_at(&bytes, 28), Some(("Bag".into(), 36)));
        assert_eq!(mono_behaviour_data_offset(&bytes), Some(36));
        assert_eq!(mono_behaviour_data_offset(&bytes[..34]), None);
    }
}
