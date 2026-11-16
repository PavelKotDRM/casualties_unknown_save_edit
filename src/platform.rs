//! Everything that depends on the operating system, kept in one place so the
//! editor can be ported by extending this module only.
//!
//! Casualties Unknown is a Windows Unity game. On Linux and the Steam Deck it
//! runs through Proton, which keeps its files inside a Wine prefix:
//! `<library>/steamapps/compatdata/<app id>/pfx`. The functions here return
//! candidates for every supported layout, most likely first:
//!
//! | What | Windows | Linux (Proton) | Native Unity layout (Linux / macOS) |
//! | --- | --- | --- | --- |
//! | Save folder | `%USERPROFILE%\AppData\LocalLow\Orsoniks\CasualtiesUnknown` | the same path inside the prefix's `drive_c/users/steamuser` | `~/.config/unity3d/…` / `~/Library/Application Support/…` |
//! | Game preferences | registry | the prefix's `user.reg` | not supported yet |
//! | Steam | `Program Files (x86)\Steam` | `~/.steam/steam`, `~/.local/share/Steam`, Flatpak | `~/Library/Application Support/Steam` |
//!
//! Game preferences backends live in [`crate::player_prefs`].

use std::fs;
use std::path::{Path, PathBuf};

/// Steam application id of Casualties Unknown.
pub const STEAM_APP_ID: u32 = 4_576_490;

/// Unity company and product names (from the game's `app.info`); Unity uses
/// them for the save folder and the preferences location.
pub const COMPANY: &str = "Orsoniks";
/// See [`COMPANY`].
pub const PRODUCT: &str = "CasualtiesUnknown";

/// Folder name of the game inside `steamapps/common`.
pub const STEAM_INSTALL_DIR: &str = "Casualties Unknown";

/// Possible save folders, most likely first.
pub fn save_dir_candidates() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if cfg!(windows) {
        if let Some(home) = dirs::home_dir() {
            dirs.push(windows_save_dir(&home));
        }
    } else {
        dirs.extend(
            proton_prefixes()
                .iter()
                .map(|prefix| windows_save_dir(&prefix.join("drive_c/users/steamuser"))),
        );
        if cfg!(target_os = "macos") {
            dirs.extend(dirs::data_dir().map(|d| d.join(COMPANY).join(PRODUCT)));
        } else {
            dirs.extend(dirs::config_dir().map(|d| d.join("unity3d").join(COMPANY).join(PRODUCT)));
        }
    }
    dirs
}

/// Unity's `persistentDataPath` of a Windows build under `home`.
fn windows_save_dir(home: &Path) -> PathBuf {
    home.join("AppData")
        .join("LocalLow")
        .join(COMPANY)
        .join(PRODUCT)
}

/// The save folder: the first candidate that exists, otherwise the most
/// likely one.
pub fn save_dir() -> Option<PathBuf> {
    let candidates = save_dir_candidates();
    candidates
        .iter()
        .find(|dir| dir.is_dir())
        .or(candidates.first())
        .cloned()
}

/// Folders where Steam itself may be installed.
pub fn steam_install_dirs() -> Vec<PathBuf> {
    if cfg!(windows) {
        let mut dirs: Vec<PathBuf> = windows_steam_path().into_iter().collect();
        for dir in ["ProgramFiles(x86)", "ProgramFiles"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(|dir| PathBuf::from(dir).join("Steam"))
        {
            if !dirs.iter().any(|d| same_dir(d, &dir)) {
                dirs.push(dir);
            }
        }
        return dirs;
    }
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    if cfg!(target_os = "macos") {
        return vec![home.join("Library/Application Support/Steam")];
    }
    vec![
        home.join(".steam/steam"),
        home.join(".local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ]
}

/// Steam's own record of where it is installed (`SteamPath`).
#[cfg(windows)]
fn windows_steam_path() -> Option<PathBuf> {
    let steam = winreg::HKCU.open_subkey(r"Software\Valve\Steam").ok()?;
    let path: String = steam.get_value("SteamPath").ok()?;
    Some(PathBuf::from(path))
}

#[cfg(not(windows))]
fn windows_steam_path() -> Option<PathBuf> {
    None
}

fn same_dir(a: &Path, b: &Path) -> bool {
    let canonical = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    canonical(a) == canonical(b)
}

/// Steam library roots: every Steam install plus the libraries listed in its
/// `libraryfolders.vdf`, without duplicates.
pub fn steam_library_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for steam in steam_install_dirs() {
        let vdf = steam.join("steamapps").join("libraryfolders.vdf");
        let mut found = fs::read_to_string(&vdf)
            .map(|text| parse_vdf_paths(&text))
            .unwrap_or_default();
        found.push(steam);
        for root in found {
            if !roots.iter().any(|r| same_dir(r, &root)) {
                roots.push(root);
            }
        }
    }
    roots
}

/// Extracts `"path"` values from a `libraryfolders.vdf` file.
pub fn parse_vdf_paths(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('"').filter(|p| !p.trim().is_empty());
            if parts.next()? != "path" {
                return None;
            }
            Some(PathBuf::from(parts.next()?.replace("\\\\", "\\")))
        })
        .collect()
}

/// Possible game installation folders in the Steam libraries.
pub fn steam_game_dirs() -> Vec<PathBuf> {
    steam_library_roots()
        .into_iter()
        .map(|root| {
            root.join("steamapps")
                .join("common")
                .join(STEAM_INSTALL_DIR)
        })
        .collect()
}

/// Existing Proton (Wine) prefixes of the game. Always empty on Windows.
pub fn proton_prefixes() -> Vec<PathBuf> {
    if cfg!(windows) {
        return Vec::new();
    }
    steam_library_roots()
        .into_iter()
        .map(|root| {
            root.join("steamapps")
                .join("compatdata")
                .join(STEAM_APP_ID.to_string())
                .join("pfx")
        })
        .filter(|prefix| prefix.is_dir())
        .collect()
}

/// Converts a path written by the game into a path on this system. Under
/// Proton the game sees the host file system as drive `Z:`.
pub fn host_path(game_path: &str) -> PathBuf {
    if !cfg!(windows)
        && let Some(rest) = game_path
            .strip_prefix("Z:")
            .or_else(|| game_path.strip_prefix("z:"))
    {
        return PathBuf::from(rest.replace('\\', "/"));
    }
    PathBuf::from(game_path)
}

/// Opens `path` in the system file manager.
pub fn open_folder(path: &Path) -> std::io::Result<()> {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .map(|_| ())
}

/// Kind of system font used as a fallback for glyphs egui does not ship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackFont {
    /// Symbols and emoji used by the interface.
    Symbols,
    /// Chinese / Japanese / Korean item names.
    Cjk,
}

/// System font files for `kind`, most preferred first. Missing files are
/// expected; the caller uses the first one that can be read.
pub fn font_candidates(kind: FallbackFont) -> Vec<PathBuf> {
    if cfg!(windows) {
        let Some(fonts) = std::env::var_os("WINDIR").map(|d| PathBuf::from(d).join("Fonts")) else {
            return Vec::new();
        };
        let files: &[&str] = match kind {
            FallbackFont::Symbols => &["seguisym.ttf"],
            FallbackFont::Cjk => &["msyh.ttc", "simsun.ttc"],
        };
        return files.iter().map(|file| fonts.join(file)).collect();
    }
    let files: &[&str] = match (kind, cfg!(target_os = "macos")) {
        (FallbackFont::Symbols, true) => &["/System/Library/Fonts/Apple Symbols.ttf"],
        (FallbackFont::Cjk, true) => &[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
        ],
        (FallbackFont::Symbols, false) => &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf",
        ],
        (FallbackFont::Cjk, false) => &[
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        ],
    };
    files.iter().map(PathBuf::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_paths_are_read_from_libraryfolders_vdf() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"SteamLibrary\\\\Games\"\n\t\t\"label\"\t\t\"\"\n\t}\n}";
        assert_eq!(
            parse_vdf_paths(vdf),
            vec![PathBuf::from(r"SteamLibrary\Games")]
        );
    }

    #[test]
    fn save_folder_follows_unity_naming() {
        let dir = windows_save_dir(Path::new("home"));
        assert!(dir.ends_with(Path::new("AppData/LocalLow/Orsoniks/CasualtiesUnknown")));
    }
}
