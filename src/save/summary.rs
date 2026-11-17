use super::{SaveError, SaveFile};
use crate::{model, schema};
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SaveSummary {
    pub character: Option<i64>,
    pub biome: Option<i64>,
    pub run_time: Option<f64>,
    pub traveled: Option<i64>,
    pub items: usize,
    /// Levels in the currently installed game's skill order.
    pub skills: Vec<i64>,
    pub blood: Option<f64>,
    pub cheated: bool,
}

impl SaveSummary {
    pub fn of(data: &Value) -> Self {
        let skills = data
            .pointer("/body/skills")
            .and_then(Value::as_object)
            .map(|skills| {
                schema::SKILLS
                    .iter()
                    .map(|definition| model::skill_progress(skills, definition.key).0)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            character: data.get("character").and_then(Value::as_i64),
            biome: data.get("biome").and_then(Value::as_i64),
            run_time: data.get("runTime").and_then(Value::as_f64),
            traveled: data.get("totalTraveled").and_then(Value::as_i64),
            items: data
                .get("items")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            skills,
            blood: data.pointer("/body/bloodVolume").and_then(Value::as_f64),
            cheated: data.get("cheatedSave").and_then(Value::as_bool) == Some(true),
        }
    }

    pub fn load(path: &Path) -> Result<Self, SaveError> {
        SaveFile::load(path).map(|file| Self::of(&file.data))
    }
}
