//! Information about the installed game: its version, engine and Steam build.
//!
//! The editor's knowledge of the game (characters, endings, layers, items,
//! world settings) is ported from a specific release, recorded in
//! [`SUPPORTED_GAME_VERSION`]. When the installed game reports another
//! version, the interface warns that new content may be missing; data read at
//! runtime (names, descriptions, item stats) still follows the installation.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::locale::DATA_DIR;

/// Game release the editor's built-in data was taken from. Update it together
/// with the tables in [`crate::schema`] after checking a new game version.
pub const SUPPORTED_GAME_VERSION: &str = "1.0.2";

/// What the editor knows about the installed game.
#[derive(Debug, Clone, Default)]
pub struct GameInfo {
    /// Installation directory.
    pub dir: PathBuf,
    /// Game version (`Application.version`). The game compares it with the
    /// `version` of its language files, so the shipped `EN.json` carries it.
    pub version: Option<String>,
    /// Unity version the game was built with.
    pub unity_version: Option<String>,
    /// Steam build, when installed through Steam.
    pub steam: Option<SteamBuild>,
}

/// Build information from the Steam app manifest (`appmanifest_<id>.acf`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SteamBuild {
    /// Steam app id.
    pub app_id: String,
    /// Steam build id of the installed files.
    pub build_id: String,
    /// When Steam last updated the game (Unix time).
    pub last_updated: Option<i64>,
    /// Installed size in bytes.
    pub size_on_disk: Option<u64>,
}

/// How the installed game relates to the editor's built-in data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compatibility {
    /// The installed version is the one the editor was made for.
    Supported,
    /// Another version is installed; built-in data may be outdated.
    Different(String),
    /// The version could not be determined.
    Unknown,
}

impl GameInfo {
    /// Reads the information available in a game installation.
    pub fn read(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            version: read_version(dir),
            unity_version: read_unity_version(dir),
            steam: read_steam_build(dir),
        }
    }

    /// Cheap fingerprint of the files a game update replaces (modification
    /// times of the language files, engine data and game code); when it
    /// changes the information should be read again.
    pub fn stamp(dir: &Path) -> Vec<Option<std::time::SystemTime>> {
        let data = dir.join(DATA_DIR);
        let lang = data.join("Lang");
        let mut paths = vec![
            data.join("globalgamemanagers"),
            data.join("resources.assets"),
            data.join("Managed").join("Assembly-CSharp.dll"),
            lang.clone(),
        ];
        if let Ok(entries) = fs::read_dir(&lang) {
            let mut files: Vec<_> = entries.flatten().map(|e| e.path()).collect();
            files.sort();
            paths.extend(files);
        }
        paths
            .iter()
            .map(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
            .collect()
    }

    /// Compares the installed version with [`SUPPORTED_GAME_VERSION`].
    pub fn compatibility(&self) -> Compatibility {
        match self.version.as_deref() {
            Some(SUPPORTED_GAME_VERSION) => Compatibility::Supported,
            Some(version) => Compatibility::Different(version.to_owned()),
            None => Compatibility::Unknown,
        }
    }
}

fn read_version(dir: &Path) -> Option<String> {
    let lang = dir.join(DATA_DIR).join("Lang");
    let mut files = vec![lang.join("EN.json")];
    if let Ok(entries) = fs::read_dir(&lang) {
        files.extend(entries.flatten().map(|entry| entry.path()));
    }
    files.into_iter().find_map(|path| {
        let text = fs::read_to_string(path).ok()?;
        let json: serde_json::Value =
            serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
        json.get("version")?
            .as_str()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    })
}

/// Unity writes its version as a C string in the header of every serialized
/// file, e.g. `2022.3.62f3`.
fn read_unity_version(dir: &Path) -> Option<String> {
    let bytes = fs::read(dir.join(DATA_DIR).join("globalgamemanagers")).ok()?;
    unity_version_in_header(&bytes[..bytes.len().min(256)])
}

fn unity_version_in_header(header: &[u8]) -> Option<String> {
    header.split(|&b| b == 0).find_map(|part| {
        let text = std::str::from_utf8(part).ok()?;
        let mut pieces = text.split('.');
        let year = pieces.next()?;
        let looks_like_version = year.len() == 4
            && year.bytes().all(|b| b.is_ascii_digit())
            && pieces.count() >= 2
            && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.');
        looks_like_version.then(|| text.to_owned())
    })
}

/// Finds the manifest of the Steam library the game is installed in
/// (`<library>\steamapps\common\<installdir>`).
fn read_steam_build(dir: &Path) -> Option<SteamBuild> {
    let install_dir = dir.file_name()?.to_str()?;
    let common = dir.parent()?;
    if !common.file_name()?.eq_ignore_ascii_case("common") {
        return None;
    }
    let steamapps = common.parent()?;
    fs::read_dir(steamapps).ok()?.flatten().find_map(|entry| {
        let name = entry.file_name();
        let name = name.to_str()?;
        if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
            return None;
        }
        let values = parse_acf(&fs::read_to_string(entry.path()).ok()?);
        values
            .get("installdir")
            .is_some_and(|d| d.eq_ignore_ascii_case(install_dir))
            .then(|| steam_build(&values))
            .flatten()
    })
}

fn steam_build(values: &HashMap<String, String>) -> Option<SteamBuild> {
    Some(SteamBuild {
        app_id: values.get("appid")?.clone(),
        build_id: values.get("buildid")?.clone(),
        last_updated: values.get("LastUpdated").and_then(|v| v.parse().ok()),
        size_on_disk: values.get("SizeOnDisk").and_then(|v| v.parse().ok()),
    })
}

/// Reads `"key" "value"` pairs of a Steam `.acf` file; the first occurrence
/// of a key wins, so top-level values take precedence over nested ones.
fn parse_acf(text: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('"').collect();
        if let [_, key, _, value, ..] = parts.as_slice() {
            values
                .entry((*key).to_owned())
                .or_insert_with(|| value.replace("\\\\", "\\"));
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_manifest_values_are_read() {
        let manifest = "\"AppState\"\n{\n\t\"appid\"\t\t\"4576490\"\n\t\"installdir\"\t\t\"Casualties Unknown\"\n\t\"LastUpdated\"\t\t\"1794661300\"\n\t\"SizeOnDisk\"\t\t\"645457789\"\n\t\"buildid\"\t\t\"26666273\"\n\t\"InstalledDepots\"\n\t{\n\t\t\"4576491\"\n\t\t{\n\t\t\t\"size\"\t\t\"1\"\n\t\t}\n\t}\n}\n";
        let build = steam_build(&parse_acf(manifest)).unwrap();
        assert_eq!(
            build,
            SteamBuild {
                app_id: "4576490".into(),
                build_id: "26666273".into(),
                last_updated: Some(1_794_661_300),
                size_on_disk: Some(645_457_789),
            }
        );
    }

    #[test]
    fn unity_version_is_found_in_a_serialized_file_header() {
        let header = b"\0\0\0\0\0\0\0\x16\0\0\0\0\x002022.3.62f3\0\x13\0\0\0";
        assert_eq!(
            unity_version_in_header(header).as_deref(),
            Some("2022.3.62f3")
        );
        assert_eq!(unity_version_in_header(b"\0\x01\x02abc\0"), None);
    }

    #[test]
    fn other_game_versions_are_reported() {
        let info = |version: Option<&str>| GameInfo {
            version: version.map(str::to_owned),
            ..GameInfo::default()
        };
        assert_eq!(
            info(Some(SUPPORTED_GAME_VERSION)).compatibility(),
            Compatibility::Supported
        );
        assert_eq!(
            info(Some("1.1.0")).compatibility(),
            Compatibility::Different("1.1.0".into())
        );
        assert_eq!(info(None).compatibility(), Compatibility::Unknown);
    }
}
