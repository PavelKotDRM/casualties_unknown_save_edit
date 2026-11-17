use crate::game_data::runtime::{RuntimeSlice, metadata};

/// A playable character, in the game's save-index order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterDef {
    pub name: &'static str,
    pub base_skills: [i64; 3],
    pub gain_multipliers: [f64; 3],
}

pub const CHARACTERS: RuntimeSlice<CharacterDef> = RuntimeSlice(|| &metadata().characters);

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
