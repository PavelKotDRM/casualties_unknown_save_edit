use super::{BACKUP_DIR_NAME, SaveError};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn backup_dir(save_path: &Path) -> PathBuf {
    save_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(BACKUP_DIR_NAME)
}

pub fn create_backup(save_path: &Path) -> Result<PathBuf, SaveError> {
    let directory = backup_dir(save_path);
    fs::create_dir_all(&directory).map_err(|error| SaveError::Io(directory.clone(), error))?;
    let stem = save_path
        .file_stem()
        .map_or_else(|| "save".into(), |stem| stem.to_string_lossy().into_owned());
    let extension = save_path.extension().map_or_else(
        || "sv".into(),
        |extension| extension.to_string_lossy().into_owned(),
    );
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let mut target = directory.join(format!("{stem}_{stamp}.{extension}"));
    let mut count = 1;
    while target.exists() {
        target = directory.join(format!("{stem}_{stamp}_{count}.{extension}"));
        count += 1;
    }
    fs::copy(save_path, &target).map_err(|error| SaveError::Io(target.clone(), error))?;
    Ok(target)
}

#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub path: PathBuf,
    pub modified: Option<SystemTime>,
    pub size: u64,
}

pub fn list_backups(save_path: &Path) -> Vec<BackupInfo> {
    let Ok(entries) = fs::read_dir(backup_dir(save_path)) else {
        return Vec::new();
    };
    let mut list: Vec<_> = entries
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| {
            let metadata = entry.metadata().ok();
            BackupInfo {
                path: entry.path(),
                modified: metadata
                    .as_ref()
                    .and_then(|metadata| metadata.modified().ok()),
                size: metadata.map_or(0, |metadata| metadata.len()),
            }
        })
        .collect();
    list.sort_by_key(|backup| std::cmp::Reverse(backup.modified));
    list
}

/// Restores a validated backup, preserving the existing target before replacing it.
pub fn restore_backup(
    backup: &Path,
    target: &Path,
) -> Result<(super::SaveFile, Option<PathBuf>), SaveError> {
    if backup.parent() != Some(backup_dir(target).as_path()) || !is_backup(backup) {
        return Err(SaveError::Io(
            backup.to_owned(),
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "backup is not in the target save's backup folder",
            ),
        ));
    }
    let mut file = super::SaveFile::load(backup)?;
    let exists = target
        .try_exists()
        .map_err(|error| SaveError::Io(target.to_owned(), error))?;
    let previous = if exists {
        Some(create_backup(target)?)
    } else {
        None
    };
    super::file::write_atomic(target, &file.disk_bytes)?;
    file.path = target.to_owned();
    Ok((file, previous))
}

pub fn is_backup(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == BACKUP_DIR_NAME)
}

/// Reconstructs the original name from the editor's timestamped backup name.
pub fn backup_source(backup: &Path) -> Option<PathBuf> {
    if !is_backup(backup) {
        return None;
    }
    let folder = backup.parent()?.parent()?;
    let stem = backup.file_stem()?.to_string_lossy();
    let parts: Vec<_> = stem.split('_').collect();
    let date_at = parts.iter().rposition(|part| {
        part.len() == 10 && part.chars().filter(|character| *character == '-').count() == 2
    })?;
    let original = parts[..date_at].join("_");
    if original.is_empty() {
        return None;
    }
    let name = match backup.extension() {
        Some(extension) => format!("{original}.{}", extension.to_string_lossy()),
        None => original,
    };
    Some(folder.join(name))
}

pub fn delete_backup(backup: &Path) -> Result<(), SaveError> {
    if !is_backup(backup) {
        return Err(SaveError::Io(
            backup.to_owned(),
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a backup"),
        ));
    }
    fs::remove_file(backup).map_err(|error| SaveError::Io(backup.to_owned(), error))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cleanup {
    KeepNewest(usize),
    OlderThanDays(u32),
}

pub fn old_backups(list: &[BackupInfo], rule: Cleanup, now: SystemTime) -> Vec<PathBuf> {
    match rule {
        Cleanup::KeepNewest(count) => list
            .iter()
            .skip(count)
            .map(|backup| backup.path.clone())
            .collect(),
        Cleanup::OlderThanDays(days) => {
            let age = std::time::Duration::from_secs(u64::from(days) * 24 * 60 * 60);
            list.iter()
                .filter(|backup| {
                    backup
                        .modified
                        .and_then(|modified| now.duration_since(modified).ok())
                        .is_some_and(|elapsed| elapsed > age)
                })
                .map(|backup| backup.path.clone())
                .collect()
        }
    }
}
