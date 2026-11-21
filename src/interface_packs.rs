//! Runtime-loaded translations for the editor interface.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

const SOURCE_STRINGS: &[&str] = include!(env!("CU_INTERFACE_STRINGS"));

/// Writes an editable language pack without requiring installed game data.
/// Existing files are never overwritten.
pub fn export_template(path: &Path) -> Result<usize, String> {
    let translations: BTreeMap<_, _> = SOURCE_STRINGS.iter().map(|&text| (text, text)).collect();
    let template = serde_json::json!({
        "code": "custom",
        "name": "Custom language",
        "translations": translations,
    });
    let mut text = serde_json::to_string_pretty(&template)
        .map_err(|error| format!("Cannot serialize interface language template: {error}"))?;
    text.push('\n');
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("Cannot create {}: {error}", path.display()))?;
    file.write_all(text.as_bytes())
        .map_err(|error| format!("Cannot write {}: {error}", path.display()))?;
    Ok(translations.len())
}

/// A language pack. Keys are the English interface strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfacePack {
    /// Stable identifier saved in the editor settings.
    pub code: String,
    /// Language name displayed in the menu.
    pub name: String,
    /// Translations indexed by the original English string.
    pub translations: BTreeMap<String, String>,
}

impl InterfacePack {
    fn parse(text: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("invalid JSON: {e}"))?;
        let code = value
            .get("code")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
            .ok_or("code must contain only ASCII letters, digits or hyphens")?;
        if code.eq_ignore_ascii_case("en") || code.eq_ignore_ascii_case("ru") {
            return Err("the built-in language codes en and ru are reserved".into());
        }
        let name = value
            .get("name")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or("name must be a non-empty string")?;
        let entries = value
            .get("translations")
            .and_then(Value::as_object)
            .ok_or("translations must be an object")?;
        let mut translations = BTreeMap::new();
        for (key, value) in entries {
            let text = value
                .as_str()
                .ok_or_else(|| format!("translation for {key:?} must be a string"))?;
            translations.insert(key.clone(), text.to_owned());
        }
        Ok(Self {
            code: code.to_owned(),
            name: name.to_owned(),
            translations,
        })
    }
}

/// Folder containing user-provided interface language JSON files.
pub fn directory() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("cu_save_editor").join("languages"))
}

/// Load all packs, retaining valid files and returning diagnostics for invalid ones.
pub fn load(directory: &Path) -> (Vec<Arc<InterfacePack>>, Vec<String>) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Vec::new(), Vec::new()),
        Err(e) => return (Vec::new(), vec![format!("{}: {e}", directory.display())]),
    };
    let mut paths = Vec::new();
    let mut errors = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry)
                if entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json")) =>
            {
                paths.push(entry.path())
            }
            Ok(_) => {}
            Err(e) => errors.push(format!("{}: {e}", directory.display())),
        }
    }
    paths.sort();
    let mut packs = Vec::new();
    let mut codes = HashSet::new();
    for path in paths {
        let pack = fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| InterfacePack::parse(&text));
        match pack {
            Ok(pack) if codes.insert(pack.code.to_ascii_lowercase()) => packs.push(Arc::new(pack)),
            Ok(pack) => errors.push(format!(
                "{}: duplicate language code {}",
                path.display(),
                pack.code
            )),
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    (packs, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::InterfaceLanguage;

    #[test]
    fn template_is_a_loadable_pack_with_labels_and_unchanged_placeholders() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("template.json");
        let count = export_template(&path).unwrap();
        let (packs, errors) = load(directory.path());
        assert!(errors.is_empty());
        let pack = &packs[0];
        assert_eq!(pack.translations.len(), count);
        for source in [
            "File",
            "Blood oxygen",
            "Brain grow sickness",
            "Weapons & ammo",
            "Includes the game's default liquids, batteries and component state. Edit contents after adding.",
            "Level {level}: {min} XP, level {next}: {max} XP",
        ] {
            assert_eq!(
                pack.translations.get(source).map(String::as_str),
                Some(source)
            );
        }
        assert!(!pack.translations.contains_key("bloodOxygen"));
        assert!(!pack.translations.contains_key("Assembly-CSharp.dll"));
        assert!(!pack.translations.contains_key("Archivo"));
    }

    #[test]
    fn template_export_preserves_existing_files_and_reports_missing_directories() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("existing.json");
        fs::write(&path, "existing translations").unwrap();
        assert!(export_template(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "existing translations");
        assert!(export_template(&directory.path().join("missing").join("template.json")).is_err());
    }

    #[test]
    fn loads_packs_and_falls_back_for_missing_or_empty_translations() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("es.json"),
            r#"{"code":"es","name":"Español","translations":{"File":"Archivo","Edit":""}}"#,
        )
        .unwrap();
        let (packs, errors) = load(dir.path());
        assert!(errors.is_empty());
        let language = InterfaceLanguage::from_code(Some("es"), Some(&packs[0]));
        assert_eq!(language.text("File"), "Archivo");
        assert_eq!(language.text("Edit"), "Edit");
        assert_eq!(language.text("Missing"), "Missing");
        assert_eq!(language.code(), "es");
    }

    #[test]
    fn reports_invalid_reserved_and_duplicate_packs() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("a.json"),
            r#"{"code":"es","name":"Spanish","translations":{}}"#,
        )
        .unwrap();
        fs::write(
            dir.path().join("b.json"),
            r#"{"code":"ES","name":"Duplicate","translations":{}}"#,
        )
        .unwrap();
        fs::write(
            dir.path().join("c.json"),
            r#"{"code":"ru","name":"Reserved","translations":{}}"#,
        )
        .unwrap();
        fs::write(
            dir.path().join("d.json"),
            r#"{"code":"fr","name":"French","translations":{"File":1}}"#,
        )
        .unwrap();
        fs::write(dir.path().join("e.json"), "{").unwrap();
        let (packs, errors) = load(dir.path());
        assert_eq!(packs.len(), 1);
        assert_eq!(errors.len(), 4);
    }

    #[test]
    fn reload_picks_up_changes_and_missing_folder_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("es.json");
        fs::write(
            &path,
            r#"{"code":"es","name":"Spanish","translations":{"File":"Archivo"}}"#,
        )
        .unwrap();
        let (first, _) = load(dir.path());
        fs::write(
            &path,
            r#"{"code":"es","name":"Spanish","translations":{"File":"Fichero"}}"#,
        )
        .unwrap();
        let (second, errors) = load(dir.path());
        assert!(errors.is_empty());
        assert_eq!(
            InterfaceLanguage::External(&first[0]).text("File"),
            "Archivo"
        );
        assert_eq!(
            InterfaceLanguage::External(&second[0]).text("File"),
            "Fichero"
        );
        let (missing, errors) = load(&dir.path().join("missing"));
        assert!(missing.is_empty());
        assert!(errors.is_empty());
    }
}
