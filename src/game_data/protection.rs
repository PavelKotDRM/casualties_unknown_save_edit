//! Protection inputs from the installed game and resource prefabs.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use dotscope::CilObject;

use super::{ItemStats, StaticValue, assets, instruction_value, metadata};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProtectionData {
    pub secondary_limbs: HashMap<String, Vec<String>>,
    pub character_bonus: HashMap<usize, f32>,
    pub base: f32,
    pub minimum: f32,
    pub default_armor: f32,
    pub default_insulation: f32,
    pub insulation: Option<super::program::ScalarProgram>,
}

pub(super) fn load(
    data_dir: &Path,
    assembly: &CilObject,
    metadata: &metadata::Metadata,
    items: &HashMap<String, ItemStats>,
) -> Result<ProtectionData, String> {
    let reader = metadata::Reader::new(assembly);
    let method = reader
        .method("Limb", "GetArmorReduction")
        .ok_or("Missing Limb.GetArmorReduction")?;
    let code: Vec<_> = method.instructions().collect();
    for (owner, field) in [
        ("Body", "bonusLimbProtection"),
        ("ProstheticInfo", "armorChange"),
        ("ItemInfo", "wearableArmor"),
    ] {
        if !code.iter().any(|instruction| {
            reader
                .field(&instruction.operand)
                .is_some_and(|(ty, name)| ty == owner && name == field)
        }) {
            return Err(format!(
                "Unsupported limb armor formula: missing {owner}.{field}"
            ));
        }
    }
    if !code.iter().any(|instruction| {
        super::instruction_string(instruction, assembly.userstrings()).as_deref()
            == Some("bonuslimbarmor")
    }) {
        return Err("Unsupported limb armor run setting".into());
    }
    let number = |instruction| match instruction_value(instruction, assembly.userstrings()) {
        Some(StaticValue::Number(value)) if value.is_finite() => Some(value),
        _ => None,
    };
    let base = code
        .first()
        .and_then(|instruction| number(instruction))
        .ok_or("Missing limb armor base")?;
    let minimum = code
        .windows(3)
        .find_map(|window| {
            if window[0].mnemonic.starts_with("ldc.") && window[1].mnemonic.starts_with("bge") {
                number(window[0])
            } else {
                None
            }
        })
        .filter(|value| *value > 0.0)
        .ok_or("Missing limb armor minimum")?;
    let ids: HashSet<_> = items
        .iter()
        .filter(|(_, item)| item.wearable)
        .map(|(id, _)| id.clone())
        .collect();
    let prefabs = assets::protection_prefabs(data_dir, assembly, &ids)?;
    let defaults = metadata::constructor_defaults(&reader, "ItemInfo");
    let insulation_method = reader
        .method("Body", "GetTotalInsulation")
        .ok_or("Missing Body.GetTotalInsulation")?;
    let insulation_code: Vec<_> = insulation_method.instructions().collect();
    let insulation = super::program::ScalarProgram::read(assembly, &insulation_code)
        .ok_or("Unsupported total insulation formula")?;
    let default = |field: &str| {
        defaults
            .get(field)
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite())
            .map(|value| value as f32)
            .ok_or_else(|| format!("Missing ItemInfo.{field} default"))
    };
    for (id, limbs) in &prefabs.secondary_limbs {
        if limbs
            .iter()
            .any(|limb| !metadata.limbs.iter().any(|known| known.object == limb))
        {
            return Err(format!("Unknown secondary limb in wearable {id}"));
        }
    }
    Ok(ProtectionData {
        secondary_limbs: prefabs.secondary_limbs,
        character_bonus: prefabs.character_bonus,
        base,
        minimum,
        default_armor: default("wearableArmor")?,
        default_insulation: default("wearableIsolation")?,
        insulation: Some(insulation),
    })
}
