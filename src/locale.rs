//! Optional display names taken from the game's own language files.
//!
//! The game ships its translations as JSON files in
//! `<game>/CasualtiesUnknown_Data/Lang/*.json`. When the installation can be
//! found, the editor shows proper item, limb and run-setting names (in any
//! language the game supports). Without it the editor falls back to the raw
//! identifiers and built-in English labels.
//!
//! The installation is located by, in order:
//! 1. the `Mono path` line of the game's `Player.log` next to the save,
//! 2. Steam library folders ([`platform::steam_game_dirs`]).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::platform;

/// Name of the game's data folder inside the installation directory.
/// Data folder of the installation, next to the executable.
pub const DATA_DIR: &str = "CasualtiesUnknown_Data";

/// Loaded translation tables.
#[derive(Debug, Default, Clone)]
pub struct Locale {
    /// Language display name, e.g. `English`.
    pub name: String,
    /// Item names and descriptions (`main` section; `<id>` and `<id>dsc`).
    main: HashMap<String, String>,
    /// Miscellaneous strings (`other` section): limb names, layer titles,
    /// run setting names (`runset<name>`), ...
    other: HashMap<String, String>,
    /// Status-effect names and descriptions (`moodles` section; `<id>` and
    /// `<id>dsc`) in file order.
    moodles: Vec<(String, String)>,
}

impl Locale {
    /// Parses a language file.
    pub fn from_json(text: &str) -> Option<Self> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let v: Value = serde_json::from_str(text).ok()?;
        let table = |key: &str| -> Vec<(String, String)> {
            v.get(key)
                .and_then(Value::as_object)
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                        .collect()
                })
                .unwrap_or_default()
        };
        Some(Self {
            name: v
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_owned(),
            main: table("main").into_iter().collect(),
            other: table("other").into_iter().collect(),
            moodles: table("moodles"),
        })
    }

    /// Loads a language file from disk.
    pub fn load(path: &Path) -> Option<Self> {
        Self::from_json(&fs::read_to_string(path).ok()?)
    }

    /// Display name of an item (`None` if unknown or empty).
    pub fn item_name(&self, id: &str) -> Option<&str> {
        non_empty(self.main.get(id))
    }

    /// Description of an item.
    pub fn item_description(&self, id: &str) -> Option<&str> {
        non_empty(self.main.get(&format!("{id}dsc")))
    }

    /// Entry of the `other` table.
    pub fn other(&self, key: &str) -> Option<&str> {
        non_empty(self.other.get(key))
    }

    pub fn moodle_name(&self, id: &str) -> Option<&str> {
        self.moodles
            .iter()
            .find(|(key, name)| key.eq_ignore_ascii_case(id) && !name.trim().is_empty())
            .map(|(_, name)| name.as_str())
    }

    /// Status effects as `(id, name, description)` in the game's order.
    pub fn moodles(&self) -> Vec<(&str, &str, Option<&str>)> {
        let description = |id: &str| {
            let key = format!("{id}dsc");
            self.moodles
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, text)| text.as_str())
                .filter(|text| !text.trim().is_empty())
        };
        self.moodles
            .iter()
            .filter(|(id, name)| {
                let description_variant = id
                    .find("dsc")
                    .is_some_and(|at| self.moodles.iter().any(|(key, _)| key == &id[..at]));
                !id.ends_with("dsc") && !description_variant && !name.trim().is_empty()
            })
            .map(|(id, name)| (id.as_str(), name.as_str(), description(id)))
            .collect()
    }
}

fn non_empty(s: Option<&String>) -> Option<&str> {
    s.map(String::as_str).filter(|s| !s.trim().is_empty())
}

/// A language file found in the game installation.
#[derive(Debug, Clone)]
pub struct LanguageFile {
    /// File stem, e.g. `EN`, `ru-RU`.
    pub code: String,
    /// Full path to the JSON file.
    pub path: PathBuf,
}

/// Lists available language files of a game installation.
pub fn list_languages(game_dir: &Path) -> Vec<LanguageFile> {
    let Ok(entries) = fs::read_dir(game_dir.join(DATA_DIR).join("Lang")) else {
        return Vec::new();
    };
    let mut list: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        })
        .filter_map(|path| {
            let code = path.file_stem()?.to_string_lossy().into_owned();
            Some(LanguageFile { code, path })
        })
        .collect();
    list.sort_by(|a, b| a.code.cmp(&b.code));
    list
}

/// Returns `true` if `dir` looks like a Casualties Unknown installation.
pub fn is_game_dir(dir: &Path) -> bool {
    dir.join(DATA_DIR).join("Lang").is_dir()
}

/// Tries to find the game installation directory automatically.
pub fn detect_game_dir(save_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(dir) = save_dir.and_then(game_dir_from_player_log) {
        return Some(dir);
    }
    platform::steam_game_dirs()
        .into_iter()
        .find(|p| is_game_dir(p))
}

/// Extracts the installation directory from `Player.log` / `Player-prev.log`.
///
/// Unity writes a line like
/// `Mono path[0] = '<game directory>/CasualtiesUnknown_Data/Managed'`.
fn game_dir_from_player_log(save_dir: &Path) -> Option<PathBuf> {
    ["Player.log", "Player-prev.log"].iter().find_map(|name| {
        let log = fs::read(save_dir.join(name)).ok()?;
        let log = String::from_utf8_lossy(&log);
        log.lines().find_map(|line| {
            let rest = line.split_once("Mono path[0] = '")?.1;
            let managed = platform::host_path(rest.split('\'').next()?);
            let dir = managed.parent()?.parent()?.to_path_buf();
            is_game_dir(&dir).then_some(dir)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_language_tables() {
        let l = Locale::from_json(
            "\u{feff}{\"name\":\"English\",\"main\":{\"bandage\":\"Dressing\",\"bandagedsc\":\"\"},\"other\":{\"Head\":\"Head\"}}",
        )
        .unwrap();
        assert_eq!(l.item_name("bandage"), Some("Dressing"));
        assert_eq!(l.item_description("bandage"), None);
        assert_eq!(l.other("Head"), Some("Head"));
    }

    #[test]
    fn moodles_pair_names_with_descriptions_in_file_order() {
        let l = Locale::from_json(
            r#"{"moodles":{"pain2":"Pain","pain2dsc":"It hurts.","pain2dscalt":"Alternate description.","pain1":"Ache","stroke":"Stroke","strokedsc":""}}"#,
        )
        .unwrap();
        assert_eq!(
            l.moodles(),
            vec![
                ("pain2", "Pain", Some("It hurts.")),
                ("pain1", "Ache", None),
                ("stroke", "Stroke", None),
            ]
        );
    }
}
