//! Reading, writing and backing up `save.sv` files.
//!
//! # File format
//!
//! Casualties Unknown stores the current run in `save.sv` inside its save
//! folder ([`platform::save_dir`]; on Windows
//! `%USERPROFILE%\AppData\LocalLow\Orsoniks\CasualtiesUnknown`).
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

// FIXME: ~590 lines covering file format (`decode`/`encode`/`validate`/`to_pretty_json`),
// new-game generation (`new_game_data`, `default_field_value`, ~90 lines of hard-coded defaults),
// the `SaveFile` handle with atomic writes, backups (`backup_dir`, `create_backup`,
// `list_backups`, `is_backup`, `backup_source`, `delete_backup`, `Cleanup`, `old_backups`) and
// `SaveSummary`. Split into `save/{format.rs, new_game.rs, file.rs, backup.rs, summary.rs}` with
// `save.rs` re-exporting the public API and `SaveError`; the backup tests should then no longer
// need to set up a full `SaveFile` round trip.

use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde_json::{Map, Value, json};

use crate::json;
use crate::model;
use crate::platform;
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

/// Returns the default save directory ([`platform::save_dir`]), e.g.
/// `%USERPROFILE%\AppData\LocalLow\Orsoniks\CasualtiesUnknown` on Windows.
pub fn default_save_dir() -> Option<PathBuf> {
    platform::save_dir()
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
    // FIXME: The generated save hard-codes desensitization 1, 10 zeroed mood entries, biome/character 0, height 100, age 20, experiment ID 1/version 0, zeroed run counters/timers/calories, rarity multipliers 1, and false run flags.
    let mut body = Map::new();
    for group in schema::BODY_GROUPS {
        for field in group.fields {
            body.insert(field.key.to_owned(), default_field_value(field));
        }
    }
    body.insert("desensitizedMult".to_owned(), json!(1.0));

    let mut skills = Map::new();
    for skill in schema::SKILLS {
        skills.insert(skill.key.to_owned(), Value::from(0));
        skills.insert(format!("exp{}", skill.key), Value::from(0.0));
        skills.insert(format!("min{}", skill.key), Value::from(0));
        skills.insert(format!("max{}", skill.key), Value::from(0));
        skills.insert(format!("{}GainMult", skill.key), Value::from(1.0));
        skills.insert(format!("{}From10", skill.key), Value::from(0.0));
    }
    crate::model::setup_character_skills(&mut skills, 0);
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
    pub modified: Option<SystemTime>,
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

/// Whether `path` is a file inside a backup folder.
pub fn is_backup(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == BACKUP_DIR_NAME)
}

/// The save a backup was made from: `…/SaveEditorBackups/save_<stamp>.sv`
/// belongs to `…/save.sv`. Returns `None` for files that are not backups.
pub fn backup_source(backup: &Path) -> Option<PathBuf> {
    if !is_backup(backup) {
        return None;
    }
    let folder = backup.parent()?.parent()?;
    let stem = backup.file_stem()?.to_string_lossy();
    // Strip `_YYYY-MM-DD_HH-MM-SS` and an optional `_<n>` counter.
    let parts: Vec<&str> = stem.split('_').collect();
    let date_at = parts
        .iter()
        .rposition(|part| part.len() == 10 && part.chars().filter(|c| *c == '-').count() == 2)?;
    let original = parts[..date_at].join("_");
    if original.is_empty() {
        return None;
    }
    let name = match backup.extension() {
        Some(ext) => format!("{original}.{}", ext.to_string_lossy()),
        None => original,
    };
    Some(folder.join(name))
}

/// Deletes a backup. Refuses to touch files outside a backup folder.
pub fn delete_backup(backup: &Path) -> Result<(), SaveError> {
    if !is_backup(backup) {
        return Err(SaveError::Io(
            backup.to_owned(),
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a backup"),
        ));
    }
    fs::remove_file(backup).map_err(|e| SaveError::Io(backup.to_owned(), e))
}

/// Rule for picking old backups to clean up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cleanup {
    /// Everything except the newest `n` backups.
    KeepNewest(usize),
    /// Backups older than this many days.
    OlderThanDays(u32),
}

/// Backups from `list` (newest first, as from [`list_backups`]) matched by
/// `rule` at time `now`. Backups with an unknown date are never matched by
/// the age rule.
pub fn old_backups(list: &[BackupInfo], rule: Cleanup, now: SystemTime) -> Vec<PathBuf> {
    match rule {
        Cleanup::KeepNewest(n) => list.iter().skip(n).map(|b| b.path.clone()).collect(),
        Cleanup::OlderThanDays(days) => {
            let age = std::time::Duration::from_secs(u64::from(days) * 24 * 60 * 60);
            list.iter()
                .filter(|b| {
                    b.modified
                        .and_then(|m| now.duration_since(m).ok())
                        .is_some_and(|d| d > age)
                })
                .map(|b| b.path.clone())
                .collect()
        }
    }
}

/// Short description of a save, used to compare backups.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SaveSummary {
    /// Character index (`character`).
    pub character: Option<i64>,
    /// Layer index (`biome`).
    pub biome: Option<i64>,
    /// Run time in seconds.
    pub run_time: Option<f64>,
    /// Distance traveled in metres.
    pub traveled: Option<i64>,
    /// Number of items.
    pub items: usize,
    /// Skill levels in [`schema::SKILLS`] order.
    pub skills: Vec<i64>,
    /// Blood volume (percent).
    pub blood: Option<f64>,
    /// Whether the game marked the save as cheated.
    pub cheated: bool,
}

impl SaveSummary {
    /// Summarises a save document.
    pub fn of(data: &Value) -> Self {
        let skills = data
            .pointer("/body/skills")
            .and_then(Value::as_object)
            .map(|skills| {
                schema::SKILLS
                    .iter()
                    .map(|def| model::skill_progress(skills, def.key).0)
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

    /// Reads and summarises a save file.
    pub fn load(path: &Path) -> Result<Self, SaveError> {
        SaveFile::load(path).map(|file| Self::of(&file.data))
    }
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
        assert_eq!(data["body"]["skills"]["STR"], 9);
        assert_eq!(data["body"]["skills"]["minSTR"], 540);
        assert_eq!(data["body"]["skills"]["expSTR"], 540.0);
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

    #[test]
    fn backups_know_their_save_and_only_backups_can_be_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("my_run.sv");
        fs::write(&path, encode(&sample(), SaveFormat::Gzip)).unwrap();
        let first = create_backup(&path).unwrap();
        let second = create_backup(&path).unwrap();
        assert!(
            second
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .ends_with("_1")
        );
        assert_eq!(backup_source(&first), Some(path.clone()));
        assert_eq!(backup_source(&second), Some(path.clone()));
        assert_eq!(backup_source(&path), None);

        assert!(delete_backup(&path).is_err());
        assert!(path.exists());
        delete_backup(&first).unwrap();
        assert_eq!(list_backups(&path).len(), 1);
    }

    #[test]
    fn summary_reads_the_run_overview() {
        let summary = SaveSummary::of(&sample());
        assert_eq!(summary.items, 1);
        assert_eq!(summary.run_time, Some(7348.6626));
        assert_eq!(summary.blood, Some(100.0));
        assert_eq!(summary.skills.first(), Some(&13));
        assert_eq!(summary.biome, None);
    }

    #[test]
    fn cleanup_rules_pick_the_old_backups() {
        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100 * 86_400);
        let day = |d: u64| Some(now - std::time::Duration::from_secs(d * 86_400));
        let list: Vec<BackupInfo> = [day(0), day(2), day(10), None]
            .into_iter()
            .enumerate()
            .map(|(i, modified)| BackupInfo {
                path: PathBuf::from(format!("b{i}")),
                modified,
                size: 0,
            })
            .collect();
        let names = |paths: Vec<PathBuf>| -> Vec<String> {
            paths.iter().map(|p| p.display().to_string()).collect()
        };
        assert_eq!(
            names(old_backups(&list, Cleanup::KeepNewest(2), now)),
            ["b2", "b3"]
        );
        assert!(old_backups(&list, Cleanup::KeepNewest(9), now).is_empty());
        assert_eq!(
            names(old_backups(&list, Cleanup::OlderThanDays(1), now)),
            ["b1", "b2"]
        );
        assert_eq!(
            names(old_backups(&list, Cleanup::OlderThanDays(0), now)),
            ["b1", "b2"]
        );
    }
}
