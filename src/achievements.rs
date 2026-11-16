//! The game's Steam achievements, read from Steam's local statistics cache.
//!
//! Steam keeps two files per game in `<Steam>/appcache/stats`:
//!
//! * `UserGameStatsSchema_<app>.bin` – every achievement with its API name,
//!   display name and description per language and a `hidden` flag;
//! * `UserGameStats_<account>_<app>.bin` – the signed-in account's progress.
//!   Achievements are bits of `ACHIEVEMENTS` stats: stat `s`, bit `b` is
//!   unlocked when bit `b` of `<s>/data` is set, and `<s>/AchievementTimes/<b>`
//!   holds the unlock time (Unix seconds).
//!
//! Both are binary `KeyValues` documents. Steam rewrites them while it runs,
//! so the editor re-reads them periodically. Achievements are read only: they
//! belong to the Steam account and cannot be changed here.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::platform;

/// A value of a binary `KeyValues` document.
#[derive(Debug, Clone, PartialEq)]
pub enum Kv {
    /// Nested section with its entries in file order.
    Section(Vec<(String, Kv)>),
    /// String value.
    Str(String),
    /// Integer value (32- or 64-bit).
    Int(i64),
    /// Floating-point value.
    Float(f32),
}

impl Kv {
    /// Entry `key` of a section.
    pub fn get(&self, key: &str) -> Option<&Kv> {
        match self {
            Kv::Section(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Entries of a section (empty for other values).
    pub fn entries(&self) -> &[(String, Kv)] {
        match self {
            Kv::Section(entries) => entries,
            _ => &[],
        }
    }

    /// String value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Kv::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Integer value (also parses numeric strings).
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Kv::Int(i) => Some(*i),
            Kv::Str(s) => s.trim().parse().ok(),
            _ => None,
        }
    }
}

/// Parses a binary `KeyValues` document into its root section.
pub fn parse_kv(bytes: &[u8]) -> Result<Kv, String> {
    let mut pos = 0;
    parse_section(bytes, &mut pos, 0)
}

fn parse_section(bytes: &[u8], pos: &mut usize, depth: usize) -> Result<Kv, String> {
    if depth > 64 {
        return Err("nesting too deep".to_owned());
    }
    let mut entries = Vec::new();
    loop {
        let Some(&kind) = bytes.get(*pos) else {
            // A truncated root is tolerated; nested sections must be closed.
            return if depth == 0 {
                Ok(Kv::Section(entries))
            } else {
                Err("unexpected end of data".to_owned())
            };
        };
        *pos += 1;
        if kind == 8 || kind == 11 {
            return Ok(Kv::Section(entries));
        }
        let key = read_cstr(bytes, pos)?;
        let value = match kind {
            0 => parse_section(bytes, pos, depth + 1)?,
            1 => Kv::Str(read_cstr(bytes, pos)?),
            2 | 4 | 6 => Kv::Int(i64::from(i32::from_le_bytes(read_n(bytes, pos)?))),
            3 => Kv::Float(f32::from_le_bytes(read_n(bytes, pos)?)),
            7 => Kv::Int(u64::from_le_bytes(read_n(bytes, pos)?) as i64),
            10 => Kv::Int(i64::from_le_bytes(read_n(bytes, pos)?)),
            other => return Err(format!("unknown value type {other} at byte {}", *pos - 1)),
        };
        entries.push((key, value));
    }
}

fn read_cstr(bytes: &[u8], pos: &mut usize) -> Result<String, String> {
    let rest = bytes.get(*pos..).unwrap_or_default();
    let len = rest
        .iter()
        .position(|&b| b == 0)
        .ok_or("unterminated string")?;
    let text = String::from_utf8_lossy(&rest[..len]).into_owned();
    *pos += len + 1;
    Ok(text)
}

fn read_n<const N: usize>(bytes: &[u8], pos: &mut usize) -> Result<[u8; N], String> {
    let chunk = bytes.get(*pos..*pos + N).ok_or("unexpected end of data")?;
    *pos += N;
    Ok(chunk.try_into().expect("slice has N bytes"))
}

/// One achievement of the game.
#[derive(Debug, Clone, PartialEq)]
pub struct Achievement {
    /// API name, e.g. `BIOPOD`.
    pub api_name: String,
    /// Statistic that holds the achievement (`1`, `2`, …).
    pub stat: String,
    /// Bit of the statistic.
    pub bit: u32,
    /// Display names by Steam language (`english`, `schinese`, …).
    pub names: Vec<(String, String)>,
    /// Descriptions by Steam language.
    pub descriptions: Vec<(String, String)>,
    /// Steam hides the name and description until it is unlocked.
    pub hidden: bool,
    /// Whether the account has it.
    pub unlocked: bool,
    /// Unlock time (Unix seconds), when known.
    pub unlocked_at: Option<i64>,
}

impl Achievement {
    /// Display name in `language` (a Steam language such as `schinese`),
    /// falling back to English.
    pub fn name(&self, language: &str) -> &str {
        localized(&self.names, language).unwrap_or(&self.api_name)
    }

    /// Description in `language`, falling back to English.
    pub fn description(&self, language: &str) -> &str {
        localized(&self.descriptions, language).unwrap_or_default()
    }
}

fn localized<'a>(texts: &'a [(String, String)], language: &str) -> Option<&'a str> {
    let find = |lang: &str| {
        texts
            .iter()
            .find(|(l, t)| l == lang && !t.trim().is_empty())
            .map(|(_, t)| t.as_str())
    };
    find(language).or_else(|| find("english"))
}

/// Achievements of the game with the account's progress.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Achievements {
    /// All achievements in the game's order.
    pub list: Vec<Achievement>,
    /// Whether the account's progress file was found; without it every
    /// achievement is shown as locked.
    pub progress_found: bool,
    /// The statistics files that were read.
    pub files: Vec<PathBuf>,
}

impl Achievements {
    /// Number of unlocked achievements.
    pub fn unlocked(&self) -> usize {
        self.list.iter().filter(|a| a.unlocked).count()
    }

    /// Reads the achievements of `app_id` from the first Steam installation
    /// that has them.
    pub fn load(app_id: u32) -> Result<Self, String> {
        let dirs = stats_dirs();
        let (dir, schema) = dirs
            .iter()
            .map(|dir| (dir, dir.join(format!("UserGameStatsSchema_{app_id}.bin"))))
            .find(|(_, schema)| schema.is_file())
            .ok_or("Steam has no achievement data for the game on this computer")?;
        let schema_bytes = fs::read(&schema).map_err(|e| format!("{}: {e}", schema.display()))?;
        let mut result = Self {
            list: parse_schema(&schema_bytes, app_id)
                .map_err(|e| format!("{}: {e}", schema.display()))?,
            files: vec![schema],
            ..Self::default()
        };
        if let Some(user) = latest_user_stats(dir, app_id) {
            let bytes = fs::read(&user).map_err(|e| format!("{}: {e}", user.display()))?;
            apply_user_stats(&mut result.list, &bytes)
                .map_err(|e| format!("{}: {e}", user.display()))?;
            result.progress_found = true;
            result.files.push(user);
        }
        Ok(result)
    }
}

/// `appcache/stats` folders of the Steam installations.
fn stats_dirs() -> Vec<PathBuf> {
    platform::steam_install_dirs()
        .into_iter()
        .map(|steam| steam.join("appcache").join("stats"))
        .filter(|dir| dir.is_dir())
        .collect()
}

/// The most recently written progress file for `app_id` (the account that
/// played last when several accounts use this computer).
fn latest_user_stats(dir: &Path, app_id: u32) -> Option<PathBuf> {
    let suffix = format!("_{app_id}.bin");
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with("UserGameStats_")
                && !name.starts_with("UserGameStatsSchema_")
                && name.ends_with(&suffix)
        })
        .max_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok())
        .map(|entry| entry.path())
}

/// Modification times of the files the achievements depend on, used to
/// re-read them only after Steam changed them.
pub fn fingerprint(app_id: u32) -> Vec<(PathBuf, Option<SystemTime>)> {
    let mut files = Vec::new();
    for dir in stats_dirs() {
        files.push(dir.join(format!("UserGameStatsSchema_{app_id}.bin")));
        files.extend(latest_user_stats(&dir, app_id));
    }
    files
        .into_iter()
        .map(|file| {
            let modified = fs::metadata(&file).and_then(|m| m.modified()).ok();
            (file, modified)
        })
        .collect()
}

/// Reads the achievement list from a schema file.
pub fn parse_schema(bytes: &[u8], app_id: u32) -> Result<Vec<Achievement>, String> {
    let root = parse_kv(bytes)?;
    let game = root
        .get(&app_id.to_string())
        .ok_or("the schema belongs to another game")?;
    let stats = game.get("stats").ok_or("the schema has no stats")?;
    let mut list = Vec::new();
    for (stat_id, stat) in stats.entries() {
        let Some(bits) = stat.get("bits") else {
            continue;
        };
        for (bit_id, bit) in bits.entries() {
            let Ok(bit_index) = bit_id.parse::<u32>() else {
                continue;
            };
            let Some(api_name) = bit.get("name").and_then(Kv::as_str) else {
                continue;
            };
            let display = bit.get("display");
            let texts = |key: &str| -> Vec<(String, String)> {
                display
                    .and_then(|d| d.get(key))
                    .map(|section| {
                        section
                            .entries()
                            .iter()
                            .filter(|(lang, _)| lang != "token")
                            .filter_map(|(lang, text)| {
                                Some((lang.clone(), text.as_str()?.to_owned()))
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            };
            list.push(Achievement {
                api_name: api_name.to_owned(),
                stat: stat_id.clone(),
                bit: bit_index,
                names: texts("name"),
                descriptions: texts("desc"),
                hidden: display
                    .and_then(|d| d.get("hidden"))
                    .and_then(Kv::as_int)
                    .is_some_and(|h| h != 0),
                unlocked: false,
                unlocked_at: None,
            });
        }
    }
    Ok(list)
}

/// Marks achievements unlocked from a user statistics file.
pub fn apply_user_stats(list: &mut [Achievement], bytes: &[u8]) -> Result<(), String> {
    let root = parse_kv(bytes)?;
    let cache = root.get("cache").ok_or("no cached statistics")?;
    for achievement in list.iter_mut() {
        let stat = cache.get(&achievement.stat);
        let data = stat
            .and_then(|s| s.get("data"))
            .and_then(Kv::as_int)
            .unwrap_or(0);
        achievement.unlocked = achievement.bit < 32 && (data as u32) & (1 << achievement.bit) != 0;
        achievement.unlocked_at = stat
            .and_then(|s| s.get("AchievementTimes"))
            .and_then(|t| t.get(&achievement.bit.to_string()))
            .and_then(Kv::as_int)
            .filter(|_| achievement.unlocked);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal binary `KeyValues` writer for tests.
    enum W<'a> {
        S(&'a str, Vec<W<'a>>),
        T(&'a str, &'a str),
        I(&'a str, i32),
    }

    fn write(nodes: &[W], out: &mut Vec<u8>) {
        for node in nodes {
            let (kind, key) = match node {
                W::S(k, _) => (0, k),
                W::T(k, _) => (1, k),
                W::I(k, _) => (2, k),
            };
            out.push(kind);
            out.extend(key.as_bytes());
            out.push(0);
            match node {
                W::S(_, children) => {
                    write(children, out);
                    out.push(8);
                }
                W::T(_, v) => {
                    out.extend(v.as_bytes());
                    out.push(0);
                }
                W::I(_, v) => out.extend(v.to_le_bytes()),
            }
        }
    }

    fn bytes(nodes: Vec<W>) -> Vec<u8> {
        let mut out = Vec::new();
        write(&nodes, &mut out);
        out.push(8);
        out
    }

    fn bit<'a>(id: &'a str, name: &'a str, title: &'a str, hidden: i32) -> W<'a> {
        W::S(
            id,
            vec![
                W::T("name", name),
                W::S(
                    "display",
                    vec![
                        W::S("name", vec![W::T("english", title), W::T("token", "T")]),
                        W::S("desc", vec![W::T("english", "Do it.")]),
                        W::I("hidden", hidden),
                    ],
                ),
            ],
        )
    }

    #[test]
    fn schema_and_progress_are_combined_bit_by_bit() {
        let schema = bytes(vec![W::S(
            "42",
            vec![W::S(
                "stats",
                vec![
                    W::S(
                        "1",
                        vec![W::S(
                            "bits",
                            vec![
                                bit("0", "FIRST", "First", 0),
                                bit("1", "SECOND", "Second", 1),
                            ],
                        )],
                    ),
                    W::S("2", vec![W::S("bits", vec![bit("0", "THIRD", "Third", 0)])]),
                ],
            )],
        )]);
        let mut list = parse_schema(&schema, 42).unwrap();
        assert_eq!(list.len(), 2 + 1);
        assert_eq!(list[1].name("russian"), "Second");
        assert!(list[1].hidden);
        assert!(parse_schema(&schema, 7).is_err());

        let user = bytes(vec![W::S(
            "cache",
            vec![
                W::I("crc", 1),
                W::S(
                    "1",
                    vec![
                        W::I("data", 0b10),
                        W::S("AchievementTimes", vec![W::I("1", 1_700_000_000)]),
                    ],
                ),
                W::S("2", vec![W::I("data", 1)]),
            ],
        )]);
        apply_user_stats(&mut list, &user).unwrap();
        assert!(!list[0].unlocked);
        assert!(list[1].unlocked);
        assert_eq!(list[1].unlocked_at, Some(1_700_000_000));
        assert!(list[2].unlocked, "bit 0 of the second stat");
        assert_eq!(list[2].unlocked_at, None);
    }

    #[test]
    fn broken_data_is_reported_instead_of_panicking() {
        assert!(parse_kv(&[0, b'a']).is_err());
        assert!(parse_kv(&[2, b'a', 0, 1]).is_err());
        assert!(parse_kv(&[0, b'a', 0, 1, b'b', 0, b'c', 0]).is_err());
    }
}
