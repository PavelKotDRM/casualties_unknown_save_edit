use std::ops::RangeInclusive;

use crate::game_data::runtime::{RuntimeSlice, metadata};

/// A playable character, in the game's save-index order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterDef {
    pub name: &'static str,
    pub base_skills: [i64; 3],
    pub gain_multipliers: [f64; 3],
    /// `Body.heightMultiplier` of the character's prefab.
    pub height_multiplier: Option<f32>,
}

pub const CHARACTERS: RuntimeSlice<CharacterDef> = RuntimeSlice(|| &metadata().characters);

/// Save keys of the identity rolled by `PlayerCamera.Start` for a new run.
pub const IDENTITY_KEYS: [&str; 4] = ["cHeight", "cAge", "cId", "cVer"];

/// Values the game can roll for `IDENTITY_KEYS[key]` when a run starts as
/// `character`; the height is scaled by the character's height multiplier.
pub fn identity_range(key: usize, character: usize) -> Option<RangeInclusive<i64>> {
    let [min, max] = *metadata().character_identity.as_ref()?.get(key)?;
    let range = min..=max - 1;
    if key != 0 {
        return Some(range);
    }
    let multiplier = CHARACTERS.get(character)?.height_multiplier?;
    // Matches the game's `(int)((float)height * heightMultiplier)`.
    let scale = |value: i64| (value as f32 * multiplier) as i64;
    Some(scale(*range.start())..=scale(*range.end()))
}

/// A body slot, in serialized `Body.slots` order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoldSlotDef {
    pub name: &'static str,
    pub locale_key: &'static str,
    pub icon: &'static str,
}

pub const HOLD_SLOTS: RuntimeSlice<HoldSlotDef> = RuntimeSlice(|| &metadata().hold_slots);

/// A skill serialized by the installed game's `Skills` type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillDef {
    pub key: &'static str,
    pub name: &'static str,
    pub locale_key: &'static str,
    pub help: &'static str,
}

pub const SKILLS: RuntimeSlice<SkillDef> = RuntimeSlice(|| &metadata().skills);
pub const LAYERS: RuntimeSlice<&str> = RuntimeSlice(|| &metadata().layers);

/// Evaluates the installed game's XP curve. Unknown curves are unavailable,
/// rather than replaced with a curve from an older game version.
pub fn try_experience_for_level(level: i32) -> Option<i64> {
    metadata().experience.as_ref()?.evaluate(level)
}

/// Experience at a level, or `None` when the installed curve cannot be evaluated.
pub fn experience_for_level(level: i32) -> Option<i64> {
    try_experience_for_level(level)
}
