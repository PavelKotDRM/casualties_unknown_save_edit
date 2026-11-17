//! Limb repair, vital signs and needs, using the installed game's defaults.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::game_data::runtime::metadata;

fn targets(
    defaults: &BTreeMap<String, Value>,
    keys: &[&str],
) -> Result<Map<String, Value>, String> {
    keys.iter()
        .map(|&key| {
            let value = defaults
                .get(key)
                .ok_or_else(|| format!("Game default for {key} is unavailable"))?;
            Ok((key.to_owned(), value.clone()))
        })
        .collect()
}

fn limb_targets(regrow: bool) -> Result<Map<String, Value>, String> {
    let mut keys = vec![
        "skinHealth",
        "muscleHealth",
        "pain",
        "bleedAmount",
        "infectionAmount",
        "furBloodAmount",
        "dislocated",
        "broken",
        "infected",
        "splinted",
        "blockedBleeding",
        "shrapnel",
    ];
    if regrow {
        keys.push("dismembered");
    }
    targets(&metadata().limb_defaults, &keys)
}

/// Heals every limb; dismembered limbs remain missing unless `regrow` is set.
pub fn heal_limbs(data: &mut Value, regrow: bool) -> Result<(), String> {
    let defaults = limb_targets(regrow)?;
    let limbs = data
        .get_mut("limbs")
        .and_then(Value::as_array_mut)
        .ok_or("This save has no limbs array")?;
    if limbs.iter().any(|limb| !limb.is_object()) {
        return Err("The save contains an invalid limb object".into());
    }
    for limb in limbs.iter_mut().filter_map(Value::as_object_mut) {
        if regrow || limb.get("dismembered").and_then(Value::as_bool) != Some(true) {
            limb.extend(defaults.clone());
        }
    }
    Ok(())
}

/// Heals one limb, preserving prosthetics and unrelated saved state.
pub fn heal_limb(limb: &mut Map<String, Value>, regrow: bool) -> Result<(), String> {
    if regrow || limb.get("dismembered").and_then(Value::as_bool) != Some(true) {
        limb.extend(limb_targets(regrow)?);
    }
    Ok(())
}

fn restore_body(data: &mut Value, keys: &[&str]) -> Result<(), String> {
    let defaults = targets(&metadata().body_defaults, keys)?;
    let body = data
        .get_mut("body")
        .and_then(Value::as_object_mut)
        .ok_or("This save has no body object")?;
    for (key, value) in defaults {
        if body.contains_key(&key) {
            body.insert(key, value);
        }
    }
    Ok(())
}

/// Restores vital signs and removes acute conditions of the body.
pub fn restore_vitals(data: &mut Value) -> Result<(), String> {
    restore_body(
        data,
        &[
            "bloodVolume",
            "bloodOxygen",
            "heartRate",
            "bloodPressure",
            "respiratoryRate",
            "fibrillationProgress",
            "consciousness",
            "brainHealth",
            "shock",
            "painShock",
            "traumaAmount",
            "internalBleeding",
            "hemothorax",
            "strokeAmount",
            "septicShock",
            "sicknessAmount",
            "radiationSickness",
            "hearingLoss",
            "temperature",
            "venomCurrent",
            "fibrillationForced",
            "hasPulmonaryEmbolism",
        ],
    )
}

/// Satisfies food, water, stamina and sleep needs.
pub fn fill_needs(data: &mut Value) -> Result<(), String> {
    restore_body(
        data,
        &[
            "hunger",
            "thirst",
            "stamina",
            "energy",
            "dirtyness",
            "wetness",
        ],
    )
}
