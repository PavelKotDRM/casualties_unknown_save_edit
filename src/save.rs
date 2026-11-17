//! Reading, writing and backing up `save.sv` files.
//!
//! The game writes gzip-compressed ASCII JSON and deletes the save after
//! loading a run. Plain JSON is also accepted and retains its encoding.

use std::fmt;
use std::path::PathBuf;

mod backup;
mod file;
mod format;
mod new_game;
mod summary;

pub use backup::*;
pub use file::SaveFile;
pub use format::{SaveFormat, decode, encode, to_pretty_json, validate};
pub use new_game::{can_create_new_game, new_game_data, try_new_game_data};
pub use summary::SaveSummary;

pub const SAVE_FILE_NAME: &str = "save.sv";
pub const BACKUP_DIR_NAME: &str = "SaveEditorBackups";

#[derive(Debug)]
pub enum SaveError {
    Io(PathBuf, std::io::Error),
    Decompress(std::io::Error),
    Json(serde_json::Error),
    NotASave(String),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(path, error) => write!(f, "{}: {error}", path.display()),
            Self::Decompress(error) => write!(f, "failed to decompress gzip data: {error}"),
            Self::Json(error) => write!(f, "invalid JSON: {error}"),
            Self::NotASave(why) => write!(f, "not a Casualties Unknown save: {why}"),
        }
    }
}

impl std::error::Error for SaveError {}

pub fn default_save_dir() -> Option<PathBuf> {
    crate::platform::save_dir()
}

pub fn default_save_path() -> Option<PathBuf> {
    default_save_dir().map(|directory| directory.join(SAVE_FILE_NAME))
}

#[cfg(test)]
mod tests;
