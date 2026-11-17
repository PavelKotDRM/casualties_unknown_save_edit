use crate::game_data::runtime::{RuntimeSlice, metadata};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RunSettingKind {
    Bool,
    Float { min: f32, max: f32, whole: bool },
    Dropdown(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunSettingDef {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: RunSettingKind,
    pub postfix: &'static str,
    pub section: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PresetValue {
    Bool(bool),
    Float(f32),
    Index(i64),
}

impl PresetValue {
    pub fn to_json(self) -> serde_json::Value {
        match self {
            Self::Bool(value) => value.into(),
            Self::Float(value) => serde_json::json!(value),
            Self::Index(value) => value.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunPreset {
    pub id: &'static str,
    pub name: &'static str,
    /// Settings supplied by the game's preset; other settings inherit from
    /// its first (default) preset.
    pub overrides: &'static [(&'static str, PresetValue)],
}

pub const RUN_PRESETS: RuntimeSlice<RunPreset> = RuntimeSlice(|| &metadata().presets);
pub const RUN_SETTINGS: RuntimeSlice<RunSettingDef> = RuntimeSlice(|| &metadata().run_settings);

pub fn run_settings_for_preset(id: &str) -> Option<Vec<serde_json::Value>> {
    let data = metadata();
    let preset = data.presets.iter().find(|preset| preset.id == id)?;
    let defaults = data.presets.first()?;
    data.run_settings
        .iter()
        .map(|setting| {
            let value = preset
                .overrides
                .iter()
                .chain(defaults.overrides)
                .find(|(key, _)| *key == setting.id)?
                .1;
            Some(serde_json::json!({"Item1": setting.id, "Item2": value.to_json()}))
        })
        .collect()
}

pub fn run_setting(id: &str) -> Option<&'static RunSettingDef> {
    RUN_SETTINGS
        .snapshot()
        .iter()
        .find(|setting| setting.id == id)
}
