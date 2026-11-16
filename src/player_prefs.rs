//! The game's Unity `PlayerPrefs`: progression kept outside the save, such as
//! character unlocks and the deepest layer reached.
//!
//! Unity keeps each preference under the name `<key>_h<hash>` (see
//! [`value_name`]). On Windows they live in the registry under
//! `HKCU\Software\Orsoniks\CasualtiesUnknown`; under Proton the same values
//! are in the Wine prefix's `user.reg`. [`PrefsStore`] hides the difference,
//! and [`store`] picks the backend for the current platform. The game sets
//! `wonchar<i>` to
//! `1` when a run is won as character `i`, which unlocks character `i + 1` in
//! the character selection (`PreRunScript`). Character 0 is always available.
//! `deepestlayer` is raised to the layer index (`biome`) the player descends
//! to (`WorldGeneration.ContinueRun` / `SaveAndExit` / `RegenerateWorld`).
//! Each reached ending sets its own flag ([`schema::ENDINGS`]); the game also
//! restores these flags from its Steam achievements on start.

use crate::schema;

/// Registry key of the game's preferences (company `Orsoniks`, product
/// `CasualtiesUnknown`, from `app.info`), relative to `HKEY_CURRENT_USER`.
pub const REGISTRY_PATH: &str = r"Software\Orsoniks\CasualtiesUnknown";

/// Value name Unity uses for a preference key.
pub fn value_name(key: &str) -> String {
    let hash = key.bytes().fold(5381u32, |hash, byte| {
        hash.wrapping_mul(33) ^ u32::from(byte)
    });
    format!("{key}_h{hash}")
}

/// Preference that records a win as `character` and unlocks the next one.
pub fn won_character_key(character: usize) -> String {
    format!("wonchar{character}")
}

/// Storage of the game's integer preferences. Each platform the game runs
/// on keeps them differently; [`store`] picks the right one.
pub trait PrefsStore {
    /// Where the preferences are kept, for messages.
    fn location(&self) -> String;
    /// Reads an integer preference (`None` if it was never set).
    fn get_int(&self, key: &str) -> Result<Option<i32>, String>;
    /// Writes an integer preference.
    fn set_int(&self, key: &str, value: i32) -> Result<(), String>;
}

/// The preferences store of the installed game on this system.
pub fn store() -> Box<dyn PrefsStore> {
    #[cfg(windows)]
    {
        Box::new(WindowsRegistry)
    }
    #[cfg(not(windows))]
    {
        match crate::platform::proton_prefixes()
            .into_iter()
            .map(|prefix| prefix.join("user.reg"))
            .find(|file| file.is_file())
        {
            Some(file) => Box::new(WineRegistry::new(file)),
            None => Box::new(Unavailable(
                "the game's settings were not found on this system".to_owned(),
            )),
        }
    }
}

/// Reads an integer preference from [`store`].
pub fn get_int(key: &str) -> Result<Option<i32>, String> {
    store().get_int(key)
}

/// Writes an integer preference to [`store`].
pub fn set_int(key: &str, value: i32) -> Result<(), String> {
    store().set_int(key, value)
}

/// Preferences in the Windows registry under `HKCU\`[`REGISTRY_PATH`].
#[cfg(windows)]
pub struct WindowsRegistry;

#[cfg(windows)]
impl PrefsStore for WindowsRegistry {
    fn location(&self) -> String {
        format!("HKCU\\{REGISTRY_PATH}")
    }

    fn get_int(&self, key: &str) -> Result<Option<i32>, String> {
        use std::io::ErrorKind;
        use winreg::HKCU;
        use winreg::enums::KEY_READ;

        let prefs = match HKCU.open_subkey_with_flags(REGISTRY_PATH, KEY_READ) {
            Ok(prefs) => prefs,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("{}: {error}", self.location())),
        };
        let name = value_name(key);
        match prefs.get_value::<u32, _>(&name) {
            Ok(value) => Ok(Some(value as i32)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(_) => prefs
                .get_value::<u64, _>(&name)
                .map(|value| Some(value as i32))
                .map_err(|error| format!("{name}: {error}")),
        }
    }

    fn set_int(&self, key: &str, value: i32) -> Result<(), String> {
        use winreg::HKCU;

        let (prefs, _) = HKCU
            .create_subkey(REGISTRY_PATH)
            .map_err(|error| format!("{}: {error}", self.location()))?;
        let name = value_name(key);
        prefs
            .set_value(&name, &(value as u32))
            .map_err(|error| format!("{name}: {error}"))
    }
}

/// Preferences in a Wine registry file (`user.reg` of a Proton prefix), as
/// written when the game runs through Proton on Linux or the Steam Deck.
/// Wine rewrites the file when the prefix shuts down, so the game must be
/// closed while it is changed.
pub struct WineRegistry {
    file: std::path::PathBuf,
}

impl WineRegistry {
    /// Uses the registry file `file` (normally `<prefix>/user.reg`).
    pub fn new(file: std::path::PathBuf) -> Self {
        Self { file }
    }

    fn section_header() -> String {
        format!("[{}]", REGISTRY_PATH.replace('\\', "\\\\"))
    }

    fn is_section(line: &str) -> bool {
        let header = Self::section_header();
        line.len() >= header.len()
            && line.is_char_boundary(header.len())
            && line[..header.len()].eq_ignore_ascii_case(&header)
    }

    /// Value of `"name"=…` lines: `dword:` or 64-bit `hex(b):` data.
    fn parse_value(data: &str) -> Option<i32> {
        if let Some(hex) = data.strip_prefix("dword:") {
            return u32::from_str_radix(hex.trim(), 16).ok().map(|v| v as i32);
        }
        let bytes = data.strip_prefix("hex(b):")?;
        let bytes: Vec<u8> = bytes
            .split(',')
            .map(|b| u8::from_str_radix(b.trim(), 16))
            .collect::<Result<_, _>>()
            .ok()?;
        let mut le = [0u8; 8];
        for (slot, byte) in le.iter_mut().zip(bytes) {
            *slot = byte;
        }
        Some(u64::from_le_bytes(le) as i32)
    }
}

impl PrefsStore for WineRegistry {
    fn location(&self) -> String {
        self.file.display().to_string()
    }

    fn get_int(&self, key: &str) -> Result<Option<i32>, String> {
        let text = std::fs::read_to_string(&self.file)
            .map_err(|error| format!("{}: {error}", self.location()))?;
        let prefix = format!("\"{}\"=", value_name(key));
        let mut in_section = false;
        for line in text.lines() {
            if line.starts_with('[') {
                in_section = Self::is_section(line);
            } else if in_section && let Some(data) = line.strip_prefix(&prefix) {
                return Ok(Self::parse_value(data.trim()));
            }
        }
        Ok(None)
    }

    fn set_int(&self, key: &str, value: i32) -> Result<(), String> {
        let text = std::fs::read_to_string(&self.file)
            .map_err(|error| format!("{}: {error}", self.location()))?;
        let prefix = format!("\"{}\"=", value_name(key));
        let entry = format!("{prefix}dword:{:08x}", value as u32);
        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        match lines.iter().position(|line| Self::is_section(line)) {
            Some(header) => {
                let end = lines[header + 1..]
                    .iter()
                    .position(|line| line.starts_with('['))
                    .map_or(lines.len(), |i| header + 1 + i);
                match lines[header + 1..end]
                    .iter()
                    .position(|line| line.starts_with(&prefix))
                {
                    Some(i) => lines[header + 1 + i] = entry,
                    None => {
                        let mut at = end;
                        while at > header + 1 && lines[at - 1].trim().is_empty() {
                            at -= 1;
                        }
                        lines.insert(at, entry);
                    }
                }
            }
            None => {
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                lines.push(String::new());
                lines.push(format!("{} {stamp}", Self::section_header()));
                lines.push(entry);
            }
        }
        let mut out = lines.join("\n");
        out.push('\n');
        std::fs::write(&self.file, out).map_err(|error| format!("{}: {error}", self.location()))
    }
}

/// Used where the game's preferences cannot be found; every access fails
/// with the stored reason, so progress is treated as unknown.
pub struct Unavailable(pub String);

impl PrefsStore for Unavailable {
    fn location(&self) -> String {
        "—".to_owned()
    }

    fn get_int(&self, _key: &str) -> Result<Option<i32>, String> {
        Err(self.0.clone())
    }

    fn set_int(&self, _key: &str, _value: i32) -> Result<(), String> {
        Err(self.0.clone())
    }
}

/// Preference holding the deepest layer index the player has descended to.
pub const DEEPEST_LAYER_KEY: &str = "deepestlayer";

/// Progress recorded by the game: unlocked characters, layers reached and
/// endings discovered.
#[derive(Debug, Clone, Default)]
pub struct GameProgress {
    /// `won[i]` is `true` once a run was won as character `i`.
    won: Vec<bool>,
    /// `endings[i]` is `true` once [`schema::ENDINGS`]`[i]` was reached.
    endings: Vec<bool>,
    /// Deepest layer index (`biome`) reached; `0` before the first descent.
    deepest_layer: i64,
    /// Why the unlock state could not be read, if it could not.
    pub error: Option<String>,
    /// Error of the last attempt to change an unlock, shown to the user.
    pub write_error: Option<String>,
}

impl GameProgress {
    /// Reads the unlock state from the game's preferences ([`store`]).
    pub fn load() -> Self {
        Self::load_from(store().as_ref())
    }

    /// Reads the unlock state from `prefs`.
    pub fn load_from(prefs: &dyn PrefsStore) -> Self {
        let mut unlocks = Self::default();
        for character in 0..schema::CHARACTERS.len() {
            match prefs.get_int(&won_character_key(character)) {
                Ok(value) => unlocks.won.push(value.is_some_and(|v| v > 0)),
                Err(error) => return Self::unknown(error),
            }
        }
        match prefs.get_int(DEEPEST_LAYER_KEY) {
            Ok(value) => unlocks.deepest_layer = i64::from(value.unwrap_or(0).max(0)),
            Err(error) => return Self::unknown(error),
        }
        for ending in &schema::ENDINGS {
            match prefs.get_int(ending.pref_key) {
                Ok(value) => unlocks.endings.push(value.is_some_and(|v| v > 0)),
                Err(error) => return Self::unknown(error),
            }
        }
        unlocks
    }

    /// Sets which endings were discovered (used to build test states).
    pub fn with_endings(mut self, endings: Vec<bool>) -> Self {
        self.endings = endings;
        self
    }

    /// Whether ending `index` of [`schema::ENDINGS`] was reached. Unknown
    /// state is treated as discovered, like character unlocks.
    pub fn is_ending_discovered(&self, index: usize) -> bool {
        !self.is_known() || self.endings.get(index).copied().unwrap_or(false)
    }

    /// Number of discovered endings, or `None` if the state could not be read.
    pub fn discovered_endings(&self) -> Option<usize> {
        self.is_known().then(|| {
            (0..schema::ENDINGS.len())
                .filter(|&i| self.is_ending_discovered(i))
                .count()
        })
    }

    /// Marks ending `index` as reached or not in the game's preferences.
    pub fn set_ending_discovered(&mut self, index: usize, discovered: bool) -> Result<(), String> {
        let ending = schema::ENDINGS
            .get(index)
            .ok_or_else(|| format!("unknown ending #{index}"))?;
        set_int(ending.pref_key, i32::from(discovered))?;
        if self.endings.len() <= index {
            self.endings.resize(index + 1, false);
        }
        self.endings[index] = discovered;
        Ok(())
    }

    fn unknown(error: String) -> Self {
        Self {
            error: Some(error),
            ..Self::default()
        }
    }

    /// Sets the deepest layer reached (used to build test states).
    pub fn with_deepest_layer(mut self, biome: i64) -> Self {
        self.deepest_layer = biome;
        self
    }

    /// Deepest layer index reached, or `None` if the state could not be read.
    pub fn deepest_layer(&self) -> Option<i64> {
        self.is_known().then_some(self.deepest_layer)
    }

    /// Whether the player has descended to layer index `biome`. Unknown
    /// state is treated as reached, like character unlocks.
    pub fn is_layer_reached(&self, biome: i64) -> bool {
        !self.is_known() || biome <= self.deepest_layer
    }

    /// Builds the state from known win flags (`won[i]`: won as character `i`).
    pub fn from_wins(won: Vec<bool>) -> Self {
        Self {
            won,
            ..Self::default()
        }
    }

    /// Whether the unlock state was read successfully.
    pub fn is_known(&self) -> bool {
        self.error.is_none()
    }

    /// Whether `character` can be selected in the game. Unknown state is
    /// treated as unlocked so the editor never blocks a choice it cannot check.
    pub fn is_unlocked(&self, character: usize) -> bool {
        character == 0 || !self.is_known() || self.won.get(character - 1).copied().unwrap_or(false)
    }

    /// Unlocks or locks `character` by setting the win flag of the previous
    /// character in the game's preferences.
    pub fn set_unlocked(&mut self, character: usize, unlocked: bool) -> Result<(), String> {
        let Some(previous) = character.checked_sub(1) else {
            return Ok(());
        };
        set_int(&won_character_key(previous), i32::from(unlocked))?;
        if self.won.len() <= previous {
            self.won.resize(previous + 1, false);
        }
        self.won[previous] = unlocked;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_names_match_the_registry_written_by_unity() {
        assert_eq!(value_name("wonchar0"), "wonchar0_h4128558139");
        assert_eq!(value_name("lastchar"), "lastchar_h2474283127");
        assert_eq!(value_name("didbiopod"), "didbiopod_h3233997235");
    }

    #[test]
    fn each_character_is_unlocked_by_a_win_with_the_previous_one() {
        let unlocks = GameProgress::from_wins(vec![true, false, false]);
        assert!(unlocks.is_unlocked(0));
        assert!(unlocks.is_unlocked(1));
        assert!(!unlocks.is_unlocked(2));
        assert!(GameProgress::from_wins(Vec::new()).is_unlocked(0));
        assert!(!GameProgress::from_wins(Vec::new()).is_unlocked(1));
    }

    #[test]
    fn only_layers_down_to_the_deepest_one_are_reached() {
        let first_run = GameProgress::from_wins(Vec::new());
        assert!(first_run.is_layer_reached(0));
        assert!(!first_run.is_layer_reached(1));
        let progress = first_run.with_deepest_layer(3);
        assert!(progress.is_layer_reached(3));
        assert!(!progress.is_layer_reached(4));
        assert!(GameProgress::unknown("no registry".into()).is_layer_reached(10));
    }

    #[test]
    fn endings_are_discovered_only_once_their_flag_is_set() {
        let progress = GameProgress::from_wins(Vec::new()).with_endings(vec![false, true]);
        assert!(!progress.is_ending_discovered(0));
        assert!(progress.is_ending_discovered(1));
        assert!(!progress.is_ending_discovered(2));
        assert_eq!(progress.discovered_endings(), Some(1));
        let unknown = GameProgress::unknown("no registry".into());
        assert!(unknown.is_ending_discovered(2));
        assert_eq!(unknown.discovered_endings(), None);
    }

    #[test]
    fn wine_registry_values_are_read_and_written_in_the_game_section() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("user.reg");
        std::fs::write(
            &file,
            "WINE REGISTRY Version 2\n\n[Software\\\\Other] 1\n\"wonchar0_h4128558139\"=dword:00000005\n\n\
             [Software\\\\Orsoniks\\\\CasualtiesUnknown] 1700000000\n#time=1\n\
             \"wonchar0_h4128558139\"=dword:00000001\n\
             \"deepestlayer_h1\"=hex(b):03,00,00,00,00,00,00,00\n\n[Software\\\\Zed] 1\n",
        )
        .unwrap();
        let prefs = WineRegistry::new(file.clone());
        assert_eq!(prefs.get_int("wonchar0"), Ok(Some(1)));
        assert_eq!(prefs.get_int("wonchar1"), Ok(None));

        prefs.set_int("wonchar0", 0).unwrap();
        prefs.set_int("wonchar1", 1).unwrap();
        assert_eq!(prefs.get_int("wonchar0"), Ok(Some(0)));
        assert_eq!(prefs.get_int("wonchar1"), Ok(Some(1)));
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("[Software\\\\Other] 1\n\"wonchar0_h4128558139\"=dword:00000005"));
        let zed = text.find("[Software\\\\Zed]").unwrap();
        assert!(text.find(&value_name("wonchar1")).unwrap() < zed);

        let empty = dir.path().join("empty.reg");
        std::fs::write(&empty, "WINE REGISTRY Version 2\n").unwrap();
        let prefs = WineRegistry::new(empty);
        prefs.set_int("didmerc", 1).unwrap();
        assert_eq!(prefs.get_int("didmerc"), Ok(Some(1)));
    }

    #[test]
    fn progress_is_unknown_when_the_preferences_are_unavailable() {
        let progress = GameProgress::load_from(&Unavailable("not here".into()));
        assert_eq!(progress.error.as_deref(), Some("not here"));
        assert!(progress.is_unlocked(2));
    }
}
