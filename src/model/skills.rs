//! Skill levels, experience boundaries and starting skills.

use crate::game_data::runtime::metadata;
use crate::json::{get_f64, get_i64, set_f64, set_i64};
use crate::schema;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// Sets a skill level and updates its experience boundaries the same way the
/// game does (`Skills.UpdateExpBoundaries`). The relative progress towards the
/// next level is kept. The derived `<KEY>From10` value is updated as well.
pub fn set_skill_level(
    skills: &mut Map<String, Value>,
    key: &str,
    level: i64,
) -> Result<(), String> {
    let level = i32::try_from(level)
        .ok()
        .filter(|level| *level >= 0)
        .ok_or("Skill level must be a nonnegative game integer")?;
    let next = level.checked_add(1).ok_or("Skill level is too large")?;
    let min = schema::try_experience_for_level(level)
        .ok_or("The game's experience curve is unavailable for this level")?;
    let max = schema::try_experience_for_level(next)
        .ok_or("The game's experience curve is unavailable for the next level")?;
    if min < 0 || max <= min || max > i64::from(i32::MAX) {
        return Err("Skill experience boundaries exceed the game's supported range".into());
    }
    let old_min = get_f64(skills, &format!("min{key}")).unwrap_or(0.0);
    let old_max = get_f64(skills, &format!("max{key}")).unwrap_or(0.0);
    let exp = get_f64(skills, &format!("exp{key}")).unwrap_or(old_min);
    let progress = if old_max > old_min {
        ((exp - old_min) / (old_max - old_min)).clamp(0.0, f64::from_bits(1.0_f64.to_bits() - 1))
    } else {
        0.0
    };
    let derived_key = format!("{key}From10");
    let derived = if skills.contains_key(&derived_key) {
        let mut fields: HashMap<_, _> = skills
            .iter()
            .filter_map(|(key, value)| Some((key.clone(), value.as_f64()? as f32)))
            .collect();
        fields.insert(key.into(), level as f32);
        Some(
            metadata()
                .skill_derived
                .get(&derived_key)
                .and_then(|getter| getter.evaluate_fields(None, &fields))
                .ok_or_else(|| format!("Game getter for {derived_key} is unavailable"))?,
        )
    } else {
        None
    };
    set_i64(skills, key, i64::from(level));
    set_i64(skills, &format!("min{key}"), min);
    set_i64(skills, &format!("max{key}"), max);
    set_f64(
        skills,
        &format!("exp{key}"),
        min as f64 + progress * (max - min) as f64,
    );
    if let Some(value) = derived {
        set_f64(skills, &derived_key, f64::from(value));
    }
    Ok(())
}

/// Gives `skills` the starting levels and gain multipliers of a character,
/// with experience at the start of each level (port of `Skills.Setup`).
pub fn setup_character_skills(
    skills: &mut Map<String, Value>,
    character: usize,
) -> Result<(), String> {
    let def = schema::CHARACTERS
        .get(character)
        .ok_or("Character metadata is unavailable")?;
    let mut updated = skills.clone();
    for (index, skill) in schema::SKILLS.iter().enumerate() {
        let key = skill.key;
        let level = def
            .base_skills
            .get(index)
            .ok_or("Character skill metadata is incomplete")?;
        let multiplier = def
            .gain_multipliers
            .get(index)
            .ok_or("Character XP multipliers are incomplete")?;
        set_skill_level(&mut updated, key, *level)?;
        let min = get_f64(&updated, &format!("min{key}"))
            .ok_or("Skill experience boundary is missing")?;
        set_f64(&mut updated, &format!("exp{key}"), min);
        set_f64(&mut updated, &format!("{key}GainMult"), *multiplier);
    }
    *skills = updated;
    Ok(())
}

/// Returns `(level, progress 0..1)` for a skill.
pub fn skill_progress(skills: &Map<String, Value>, key: &str) -> (i64, f64) {
    let level = get_i64(skills, key).unwrap_or(0);
    let min = get_f64(skills, &format!("min{key}")).unwrap_or(0.0);
    let max = get_f64(skills, &format!("max{key}")).unwrap_or(0.0);
    let exp = get_f64(skills, &format!("exp{key}")).unwrap_or(min);
    let p = if max > min {
        (exp - min) / (max - min)
    } else {
        0.0
    };
    (level, p.clamp(0.0, 1.0))
}
