//! Knowledge about the meaning of save fields.
//!
//! Each known field gets a human-readable label, a description, a sensible
//! range and a hint about which values are "good". The UI uses this to render
//! gauges and colour-code values similar to the in-game health screen.
//! Ranges mirror the clamps used by the game code where they exist; values
//! outside the range can still be entered through the numeric editors.

/// Which values of a stat are healthy; used for colour coding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Good {
    /// Higher is better (health, blood volume, ...).
    High,
    /// Lower is better (pain, shock, bleeding, ...).
    Low,
    /// Values close to the given target are best (heart rate, temperature).
    Near(f32),
    /// No judgement; displayed neutrally.
    Neutral,
}

/// How a field is edited and visualised.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// Bounded number, shown as a gauge with a slider.
    Gauge,
    /// Free number (timers, physics constants), shown as a drag value.
    Number,
    /// Integer counter.
    Int,
    /// Boolean flag.
    Flag,
}

/// Metadata of a single field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldSpec {
    /// JSON key.
    pub key: &'static str,
    /// Display label.
    pub label: &'static str,
    /// Tooltip text.
    pub help: &'static str,
    /// Editing / display kind.
    pub kind: Kind,
    /// Lower bound of the gauge/slider.
    pub min: f32,
    /// Upper bound of the gauge/slider.
    pub max: f32,
    /// Unit suffix shown after the value.
    pub unit: &'static str,
    /// Which values are healthy.
    pub good: Good,
}

/// A titled group of body fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldGroup {
    /// Group title.
    pub title: &'static str,
    /// Icon shown before the title.
    pub icon: &'static str,
    /// Fields of the group in display order.
    pub fields: &'static [FieldSpec],
}

mod body;
mod character;
mod endings;
mod limbs;
mod run;

pub use body::{BODY_GROUPS, body_field};
pub use character::{
    CHARACTERS, CharacterDef, HOLD_SLOTS, HoldSlotDef, IDENTITY_KEYS, LAYERS, SKILLS, SkillDef,
    experience_for_level, identity_range, try_experience_for_level,
};
pub use endings::{ENDINGS, EndingDef};
pub use limbs::{LIMB_FIELDS, LIMBS, LimbDef, limb_field};
pub use run::{
    PresetValue, RUN_PRESETS, RUN_SETTINGS, RunPreset, RunSettingDef, RunSettingKind, run_setting,
    run_settings_for_preset,
};
