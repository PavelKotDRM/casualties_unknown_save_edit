//! Persistent editor settings.

use crate::save;
use serde_json::{Value, json};
use std::path::PathBuf;

/// Persistent user settings (`cu_save_editor/config.json` in the OS configuration folder).
pub(super) struct Config {
    pub(super) game_dir: Option<PathBuf>,
    pub(super) language: Option<String>,
    pub(super) interface_language: String,
    /// Re-read the save, unlocks, achievements and game data periodically.
    pub(super) auto_refresh: bool,
    pub(super) spoilers: crate::ui::spoilers::Spoilers,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            game_dir: None,
            language: None,
            interface_language: "en".to_owned(),
            auto_refresh: true,
            spoilers: Default::default(),
        }
    }
}

impl Config {
    fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("cu_save_editor").join("config.json"))
    }

    pub(super) fn load() -> (Self, Option<String>) {
        let Some(v) = Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        else {
            return (Self::default(), None);
        };
        let (spoilers, error) = match v
            .get("spoilers")
            .cloned()
            .map(crate::ui::spoilers::Spoilers::from_json)
            .transpose()
        {
            Ok(state) => (state.unwrap_or_default(), None),
            Err(error) => (Default::default(), Some(error.to_string())),
        };
        (
            Self {
                game_dir: v.get("gameDir").and_then(Value::as_str).map(PathBuf::from),
                language: v.get("language").and_then(Value::as_str).map(str::to_owned),
                interface_language: v
                    .get("interfaceLanguage")
                    .and_then(Value::as_str)
                    .unwrap_or("en")
                    .to_owned(),
                auto_refresh: v
                    .get("autoRefresh")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
                spoilers,
            },
            error,
        )
    }

    pub(super) fn store(&self) -> Result<(), String> {
        let path = Self::path().ok_or("Cannot determine the editor settings folder")?;
        let v = json!({
            "gameDir": self.game_dir.as_ref().map(|p| p.to_string_lossy()),
            "language": self.language,
            "interfaceLanguage": self.interface_language,
            "autoRefresh": self.auto_refresh,
            "spoilers": self.spoilers.to_json(),
        });
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&path, save::to_pretty_json(&v))
            .map_err(|e| format!("{}: {e}", path.display()))
    }
}
