//! Reading, writing and backing up `save.sv` files.
//!
//! # File format
//!
//! Casualties Unknown stores the current run in
//! `%USERPROFILE%\AppData\LocalLow\Orsoniks\CasualtiesUnknown\save.sv`.
//! The file is a gzip stream containing a single line of ASCII JSON produced
//! by Newtonsoft.Json (`SaveSystem.SaveGame` in the game code).
//!
//! The editor also accepts an uncompressed JSON file (handy for hand-edited
//! or exported saves) and remembers which format was loaded so it can be
//! written back in the same format.
//!
//! **Note:** the game deletes `save.sv` right after loading a run, so the
//! file only exists while the game is closed (or in the main menu) after
//! "Save & Quit".

use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde_json::{Map, Value, json};

use crate::json;
use crate::schema::{self, Good, Kind};

/// File name of the save written by the game.
pub const SAVE_FILE_NAME: &str = "save.sv";

/// Name of the folder (next to the save) where backups are stored.
pub const BACKUP_DIR_NAME: &str = "SaveEditorBackups";

/// Errors that can occur while reading or writing a save.
#[derive(Debug)]
pub enum SaveError {
    /// Filesystem error, with the path involved.
    Io(PathBuf, std::io::Error),
    /// The gzip stream could not be decompressed.
    Decompress(std::io::Error),
    /// The content is not valid JSON.
    Json(serde_json::Error),
    /// The JSON is valid but does not look like a Casualties Unknown save.
    NotASave(String),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Io(path, e) => write!(f, "{}: {e}", path.display()),
            SaveError::Decompress(e) => write!(f, "failed to decompress gzip data: {e}"),
            SaveError::Json(e) => write!(f, "invalid JSON: {e}"),
            SaveError::NotASave(why) => write!(f, "not a Casualties Unknown save: {why}"),
        }
    }
}

impl std::error::Error for SaveError {}

/// On-disk encoding of a save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFormat {
    /// gzip-compressed JSON, the format used by the game (`save.sv`).
    Gzip,
    /// Plain JSON text.
    PlainJson,
}

/// Returns the default save directory:
/// `%USERPROFILE%\AppData\LocalLow\Orsoniks\CasualtiesUnknown`.
pub fn default_save_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| {
        home.join("AppData")
            .join("LocalLow")
            .join("Orsoniks")
            .join("CasualtiesUnknown")
    })
}

/// Returns the default save file path (see [`default_save_dir`]).
pub fn default_save_path() -> Option<PathBuf> {
    default_save_dir().map(|d| d.join(SAVE_FILE_NAME))
}

/// Decodes raw file bytes into JSON, auto-detecting gzip by its magic bytes.
pub fn decode(bytes: &[u8]) -> Result<(Value, SaveFormat), SaveError> {
    let (text, format) = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut text = Vec::new();
        GzDecoder::new(bytes)
            .read_to_end(&mut text)
            .map_err(SaveError::Decompress)?;
        (text, SaveFormat::Gzip)
    } else {
        (bytes.to_vec(), SaveFormat::PlainJson)
    };
    let text = text.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&text);
    let value: Value = serde_json::from_slice(text).map_err(SaveError::Json)?;
    validate(&value)?;
    Ok((value, format))
}

/// Performs a light sanity check that the JSON looks like a save.
pub fn validate(value: &Value) -> Result<(), SaveError> {
    let obj = value
        .as_object()
        .ok_or_else(|| SaveError::NotASave("the root is not a JSON object".into()))?;
    for key in ["body", "limbs", "items"] {
        if !obj.contains_key(key) {
            return Err(SaveError::NotASave(format!("missing \"{key}\" section")));
        }
    }
    Ok(())
}

/// Creates a clean save document using the game's normal world preset.
pub fn new_game_data() -> Value {
    let mut body = Map::new();
    for group in schema::BODY_GROUPS {
        for field in group.fields {
            body.insert(field.key.to_owned(), default_field_value(field));
        }
    }
    body.insert("desensitizedMult".to_owned(), json!(1.0));

    let mut skills = Map::new();
    for skill in schema::SKILLS {
        skills.insert(skill.key.to_owned(), Value::from(10));
        skills.insert(format!("exp{}", skill.key), Value::from(0.0));
        skills.insert(format!("min{}", skill.key), Value::from(0));
        skills.insert(format!("max{}", skill.key), Value::from(0));
        skills.insert(format!("{}GainMult", skill.key), Value::from(1.0));
        skills.insert(format!("{}From10", skill.key), Value::from(0.0));
    }
    for skill in schema::SKILLS {
        crate::model::set_skill_level(&mut skills, skill.key, 10);
    }
    body.insert("skills".to_owned(), Value::Object(skills));

    let limbs: Vec<_> = schema::LIMBS
        .iter()
        .map(|_| {
            let mut limb = Map::new();
            for field in schema::LIMB_FIELDS {
                limb.insert(field.key.to_owned(), default_field_value(field));
            }
            limb.insert("prostheticInfo".to_owned(), json!({}));
            Value::Object(limb)
        })
        .collect();
    let limb_components = vec![json!({}); schema::LIMBS.len()];
    let mood_history = vec![Value::from(0.0); 10];
    let data = json!({
        "body": body,
        "limbs": limbs,
        "items": [],
        "itemComponents": [],
        "bodyComponents": {},
        "limbComponents": limb_components,
        "biome": 0,
        "endlessLoops": 0,
        "totalTraveled": 0.0,
        "runTime": 0.0,
        "cHeight": 100,
        "cAge": 20,
        "cId": 1,
        "cVer": 0,
        "savedRecipeData": {"saved": []},
        "lastHappiness": mood_history,
        "trapRarity": 1.0,
        "lootRarity": 1.0,
        "caloriesConsumed": 0.0,
        "character": 0,
        "isMercRun": false,
        "cheatedSave": false,
        "runSettings": schema::run_settings_for_preset("normal")
            .expect("the normal game preset must exist"),
    });
    data
}

fn default_field_value(field: &schema::FieldSpec) -> Value {
    match field.kind {
        Kind::Gauge => {
            let value = match field.good {
                Good::High => field.max,
                Good::Low => field.min,
                Good::Near(value) => value,
                Good::Neutral => 0.0,
            };
            json!(value)
        }
        Kind::Number => json!(0.0),
        Kind::Int => Value::from(0),
        Kind::Flag => Value::Bool(false),
    }
}

/// Encodes a save for writing to disk.
///
/// JSON is written compactly (like the game does) with all non-ASCII
/// characters escaped; see [`json::to_ascii_json`].
pub fn encode(value: &Value, format: SaveFormat) -> Vec<u8> {
    let text = json::to_ascii_json(value);
    match format {
        SaveFormat::PlainJson => text.into_bytes(),
        SaveFormat::Gzip => {
            let mut enc = GzEncoder::new(Vec::new(), Compression::best());
            enc.write_all(text.as_bytes())
                .expect("writing to a Vec cannot fail");
            enc.finish().expect("writing to a Vec cannot fail")
        }
    }
}

/// Pretty-printed, human-readable JSON (used for export and the raw editor).
pub fn to_pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).expect("serialising a Value cannot fail")
}

/// A save loaded into memory.
#[derive(Debug, Clone)]
pub struct SaveFile {
    /// Where the save was loaded from (and will be written to).
    pub path: PathBuf,
    /// On-disk encoding.
    pub format: SaveFormat,
    /// The editable document.
    pub data: Value,
    /// Raw bytes as they were on disk when loaded or last written; used to
    /// detect whether the game modified the file in the meantime.
    pub disk_bytes: Vec<u8>,
}

impl SaveFile {
    /// Loads a save from disk.
    pub fn load(path: &Path) -> Result<Self, SaveError> {
        let bytes = fs::read(path).map_err(|e| SaveError::Io(path.to_owned(), e))?;
        let (data, format) = decode(&bytes)?;
        Ok(Self {
            path: path.to_owned(),
            format,
            data,
            disk_bytes: bytes,
        })
    }

    /// Returns `true` if the file on disk differs from what was loaded or
    /// last written by the editor (e.g. the game saved again).
    pub fn changed_on_disk(&self) -> bool {
        fs::read(&self.path).map_or(true, |b| b != self.disk_bytes)
    }

    /// Writes [`Self::data`] back to [`Self::path`].
    ///
    /// The write is atomic: data goes to a temporary file in the same
    /// directory which then replaces the original. If the file on disk has
    /// changed since it was loaded, it is backed up first so no game progress
    /// can be lost. Returns the path of that extra backup, if one was made.
    pub fn write(&mut self) -> Result<Option<PathBuf>, SaveError> {
        let extra_backup = if self.path.exists() && self.changed_on_disk() {
            Some(create_backup(&self.path)?)
        } else {
            None
        };
        let bytes = encode(&self.data, self.format);
        write_atomic(&self.path, &bytes)?;
        self.disk_bytes = bytes;
        Ok(extra_backup)
    }

    /// Writes the save to another location (changing [`Self::path`]).
    pub fn write_as(&mut self, path: &Path, format: SaveFormat) -> Result<(), SaveError> {
        let bytes = encode(&self.data, format);
        write_atomic(path, &bytes)?;
        self.path = path.to_owned();
        self.format = format;
        self.disk_bytes = bytes;
        Ok(())
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), SaveError> {
    let io = |e| SaveError::Io(path.to_owned(), e);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes).map_err(io)?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        io(e)
    })
}

/// Directory where backups of `save_path` are stored.
pub fn backup_dir(save_path: &Path) -> PathBuf {
    save_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(BACKUP_DIR_NAME)
}

/// Copies `save_path` into [`backup_dir`] with a timestamped name such as
/// `save_2026-11-15_08-30-00.sv` and returns the backup path.
pub fn create_backup(save_path: &Path) -> Result<PathBuf, SaveError> {
    let dir = backup_dir(save_path);
    fs::create_dir_all(&dir).map_err(|e| SaveError::Io(dir.clone(), e))?;
    let stem = save_path
        .file_stem()
        .map_or_else(|| "save".into(), |s| s.to_string_lossy().into_owned());
    let ext = save_path
        .extension()
        .map_or_else(|| "sv".into(), |s| s.to_string_lossy().into_owned());
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let mut target = dir.join(format!("{stem}_{stamp}.{ext}"));
    let mut n = 1;
    while target.exists() {
        target = dir.join(format!("{stem}_{stamp}_{n}.{ext}"));
        n += 1;
    }
    fs::copy(save_path, &target).map_err(|e| SaveError::Io(target.clone(), e))?;
    Ok(target)
}

/// Information about an existing backup file.
#[derive(Debug, Clone)]
pub struct BackupInfo {
    /// Full path of the backup.
    pub path: PathBuf,
    /// Last modification time.
    pub modified: Option<std::time::SystemTime>,
    /// File size in bytes.
    pub size: u64,
}

/// Lists backups for `save_path`, newest first.
pub fn list_backups(save_path: &Path) -> Vec<BackupInfo> {
    let Ok(entries) = fs::read_dir(backup_dir(save_path)) else {
        return Vec::new();
    };
    let mut list: Vec<BackupInfo> = entries
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| {
            let meta = e.metadata().ok();
            BackupInfo {
                path: e.path(),
                modified: meta.as_ref().and_then(|m| m.modified().ok()),
                size: meta.map_or(0, |m| m.len()),
            }
        })
        .collect();
    list.sort_by_key(|a| std::cmp::Reverse(a.modified));
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({
            "body": {"bloodVolume": 100.0, "corpsesSeen": 12, "skills": {"STR": 13}},
            "limbs": [{"skinHealth": 88.66524}],
            "items": [{"id": "waterbottle", "wearSlot": null, "label": "Фляга"}],
            "runTime": 7348.6626
        })
    }

    #[test]
    fn new_game_data_has_a_clean_inventory_and_complete_known_settings() {
        let data = new_game_data();

        validate(&data).unwrap();
        assert_eq!(data["items"], json!([]));
        assert_eq!(data["itemComponents"], json!([]));
        assert_eq!(data["limbs"].as_array().unwrap().len(), schema::LIMBS.len());
        assert_eq!(
            data["runSettings"].as_array().unwrap().len(),
            schema::RUN_SETTINGS.len()
        );
        assert_eq!(data["body"]["skills"]["STR"], 10);
        assert_eq!(data["body"]["skills"]["minSTR"], 640);
        assert_eq!(data["body"]["bloodVolume"], 100.0);
        assert_eq!(data["body"]["desensitizedMult"], 1.0);
        assert_eq!(data["limbs"][0]["prostheticInfo"], json!({}));
        assert_eq!(
            data["limbComponents"].as_array().unwrap().len(),
            schema::LIMBS.len()
        );
        assert_eq!(data["lastHappiness"].as_array().unwrap().len(), 10);
    }

    #[test]
    fn gzip_round_trip_preserves_document_and_order() {
        let value = sample();
        let bytes = encode(&value, SaveFormat::Gzip);
        assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
        let (decoded, format) = decode(&bytes).unwrap();
        assert_eq!(format, SaveFormat::Gzip);
        assert_eq!(decoded, value);
        let keys: Vec<_> = decoded.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["body", "limbs", "items", "runTime"]);
        // Untouched numbers are written exactly as the game wrote them.
        let text = json::to_ascii_json(&decoded);
        assert!(text.contains("\"skinHealth\":88.66524"));
        assert!(text.contains("\"bloodVolume\":100.0"));
    }

    #[test]
    fn plain_json_with_bom_is_accepted() {
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend(serde_json::to_vec(&sample()).unwrap());
        let (_, format) = decode(&bytes).unwrap();
        assert_eq!(format, SaveFormat::PlainJson);
    }

    #[test]
    fn non_save_json_is_rejected() {
        assert!(matches!(decode(b"{\"a\":1}"), Err(SaveError::NotASave(_))));
        assert!(matches!(decode(b"not json"), Err(SaveError::Json(_))));
    }

    #[test]
    fn write_backs_up_externally_modified_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SAVE_FILE_NAME);
        fs::write(&path, encode(&sample(), SaveFormat::Gzip)).unwrap();

        let first = create_backup(&path).unwrap();
        assert!(first.starts_with(backup_dir(&path)));

        let mut save = SaveFile::load(&path).unwrap();
        save.data["runTime"] = json!(1.0);
        assert_eq!(
            save.write().unwrap(),
            None,
            "unchanged file needs no extra backup"
        );

        // Simulate the game writing a new save while the editor is open.
        fs::write(&path, encode(&sample(), SaveFormat::Gzip)).unwrap();
        let extra = save.write().unwrap().expect("extra backup expected");
        assert!(extra.exists());
        assert_eq!(SaveFile::load(&path).unwrap().data["runTime"], json!(1.0));
        assert_eq!(list_backups(&path).len(), 2);
    }
}
