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
#[derive(Debug, Clone, Copy)]
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

const fn gauge(
    key: &'static str,
    label: &'static str,
    min: f32,
    max: f32,
    unit: &'static str,
    good: Good,
    help: &'static str,
) -> FieldSpec {
    FieldSpec {
        key,
        label,
        help,
        kind: Kind::Gauge,
        min,
        max,
        unit,
        good,
    }
}

const fn num(key: &'static str, label: &'static str, help: &'static str) -> FieldSpec {
    FieldSpec {
        key,
        label,
        help,
        kind: Kind::Number,
        min: f32::NEG_INFINITY,
        max: f32::INFINITY,
        unit: "",
        good: Good::Neutral,
    }
}

const fn int(key: &'static str, label: &'static str, help: &'static str) -> FieldSpec {
    FieldSpec {
        key,
        label,
        help,
        kind: Kind::Int,
        min: f32::NEG_INFINITY,
        max: f32::INFINITY,
        unit: "",
        good: Good::Neutral,
    }
}

const fn flag(key: &'static str, label: &'static str, help: &'static str) -> FieldSpec {
    FieldSpec {
        key,
        label,
        help,
        kind: Kind::Flag,
        min: 0.0,
        max: 1.0,
        unit: "",
        good: Good::Neutral,
    }
}

/// A titled group of body fields.
#[derive(Debug, Clone, Copy)]
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
mod limbs;
mod run;

pub use body::{BODY_GROUPS, body_field};
pub use character::{CHARACTERS, LAYERS, SKILLS, SkillDef, experience_for_level};
pub use limbs::{LIMB_FIELDS, LIMBS, LimbDef, limb_field};
pub use run::{RUN_SETTINGS, RunSettingDef, RunSettingKind, run_setting};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experience_matches_values_written_by_the_game() {
        // Taken from a real save: STR 13 -> [1062, 1255), RES 14 -> [1255, 1484), INT 12 -> [897, 1062).
        assert_eq!(experience_for_level(12), 897);
        assert_eq!(experience_for_level(13), 1062);
        assert_eq!(experience_for_level(14), 1255);
        assert_eq!(experience_for_level(15), 1484);
        assert_eq!(experience_for_level(0), 0);
    }
}
