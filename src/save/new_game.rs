use super::SaveError;
use crate::{game_data::runtime, schema};
use serde_json::{Map, Value, json};

/// A template requires runtime prefab structure, initializer values and the
/// game's character/XP definitions; none have built-in fallbacks.
pub fn can_create_new_game() -> bool {
    let metadata = runtime::metadata();
    !metadata.body_groups.is_empty()
        && !metadata.limbs.is_empty()
        && !metadata.characters.is_empty()
        && metadata.experience.is_some()
        && metadata.character_identity.is_some()
        && metadata.mood_history_length.is_some()
        && schema::run_settings_for_preset(metadata.presets.first().map_or("", |preset| preset.id))
            .is_some()
}

pub fn try_new_game_data() -> Result<Value, SaveError> {
    let unavailable =
        || SaveError::NotASave("New-run generation requires supported installed game data".into());
    if !can_create_new_game() {
        return Err(unavailable());
    }
    let metadata = runtime::metadata();
    let identity = metadata.character_identity.ok_or_else(unavailable)?;
    let mut root: Map<_, _> = metadata.root_defaults.clone().into_iter().collect();
    let fields = |specs: &[schema::FieldSpec],
                  defaults: &std::collections::BTreeMap<String, Value>|
     -> Result<Map<String, Value>, SaveError> {
        specs
            .iter()
            .map(|field| {
                Ok((
                    field.key.into(),
                    defaults.get(field.key).cloned().ok_or_else(unavailable)?,
                ))
            })
            .collect()
    };
    let body_specs: Vec<_> = metadata
        .body_groups
        .iter()
        .flat_map(|group| group.fields.iter().copied())
        .collect();
    let mut body = fields(&body_specs, &metadata.body_defaults)?;
    let mut skills: Map<_, _> = metadata.skill_defaults.clone().into_iter().collect();
    let character = metadata.characters.first().ok_or_else(unavailable)?;
    let experience = metadata.experience.as_ref().ok_or_else(unavailable)?;
    for (index, skill) in metadata.skills.iter().enumerate() {
        let level = *character.base_skills.get(index).ok_or_else(unavailable)?;
        let min = experience
            .evaluate(i32::try_from(level).map_err(|_| unavailable())?)
            .ok_or_else(unavailable)?;
        let max = experience
            .evaluate(i32::try_from(level + 1).map_err(|_| unavailable())?)
            .ok_or_else(unavailable)?;
        let offset = *metadata
            .skill_offsets
            .get(skill.key)
            .ok_or_else(unavailable)?;
        skills.insert(skill.key.into(), level.into());
        skills.insert(format!("exp{}", skill.key), json!(min as f64));
        skills.insert(format!("min{}", skill.key), min.into());
        skills.insert(format!("max{}", skill.key), max.into());
        skills.insert(
            format!("{}GainMult", skill.key),
            json!(character.gain_multipliers[index]),
        );
        skills.insert(format!("{}From10", skill.key), json!(level as f32 - offset));
    }
    body.insert("skills".into(), skills.into());
    let mut limb = fields(&metadata.limb_fields, &metadata.limb_defaults)?;
    // Missing members deserialize to the default value of the game's struct.
    limb.insert("prostheticInfo".into(), json!({}));
    root.insert("body".into(), body.into());
    root.insert(
        "limbs".into(),
        json!(vec![Value::Object(limb); metadata.limbs.len()]),
    );
    root.insert("items".into(), json!([]));
    root.insert("itemComponents".into(), json!([]));
    root.insert("bodyComponents".into(), json!({}));
    root.insert(
        "limbComponents".into(),
        json!(vec![json!({}); metadata.limbs.len()]),
    );
    root.insert("savedRecipeData".into(), json!({"saved": []}));
    root.insert(
        "lastHappiness".into(),
        json!(vec![
            0.0;
            metadata.mood_history_length.ok_or_else(unavailable)?
        ]),
    );
    for (key, value) in ["cHeight", "cAge", "cId", "cVer"].into_iter().zip(identity) {
        root.insert(key.into(), value.into());
    }
    root.insert(
        "runSettings".into(),
        json!(schema::run_settings_for_preset(metadata.presets[0].id).ok_or_else(unavailable)?),
    );
    Ok(root.into())
}

/// Generates a new save or reports unsupported installation metadata.
pub fn new_game_data() -> Result<Value, SaveError> {
    try_new_game_data()
}
