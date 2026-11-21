use std::fs;
use std::path::{Path, PathBuf};

use super::{SaveError, SaveFormat, create_backup, decode, encode};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct SaveFile {
    pub path: PathBuf,
    pub format: SaveFormat,
    pub data: Value,
    /// Bytes last observed on disk, used to detect external writes.
    pub disk_bytes: Vec<u8>,
}

impl SaveFile {
    pub fn load(path: &Path) -> Result<Self, SaveError> {
        let bytes = fs::read(path).map_err(|error| SaveError::Io(path.to_owned(), error))?;
        let (data, format) = decode(&bytes)?;
        Ok(Self {
            path: path.to_owned(),
            format,
            data,
            disk_bytes: bytes,
        })
    }

    pub fn changed_on_disk(&self) -> bool {
        fs::read(&self.path).map_or(true, |bytes| bytes != self.disk_bytes)
    }

    /// Saves atomically, preserving a concurrent game write in a backup.
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

    pub fn write_as(&mut self, path: &Path, format: SaveFormat) -> Result<(), SaveError> {
        let bytes = encode(&self.data, format);
        write_atomic(path, &bytes)?;
        self.path = path.to_owned();
        self.format = format;
        self.disk_bytes = bytes;
        Ok(())
    }
}

pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), SaveError> {
    let io = |error| SaveError::Io(path.to_owned(), error);
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    fs::write(&temporary, bytes).map_err(io)?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        io(error)
    })
}
