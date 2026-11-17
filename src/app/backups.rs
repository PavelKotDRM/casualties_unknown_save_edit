//! Application backups operations.

use super::*;

impl EditorApp {
    /// Save whose backups the browser shows: the open save (or, for an open
    /// backup, the save it was made from), else the game's default save.
    pub(in crate::app) fn backup_target(&self) -> Option<PathBuf> {
        match self.save.as_ref().map(|s| &s.path) {
            Some(path) if save::is_backup(path) => save::backup_source(path),
            Some(path) => Some(path.clone()),
            None => save::default_save_path(),
        }
    }

    pub(in crate::app) fn open_backups(&mut self) {
        let target = self.backup_target();
        self.backups.show_for(target);
    }

    pub(in crate::app) fn manual_backup(&mut self) {
        let Some(path) = self.backup_target().filter(|p| p.is_file()) else {
            return;
        };
        match save::create_backup(&path) {
            Ok(b) => {
                self.set_status(format!("Backup created: {}", b.display()), Level::Success);
                self.last_backup = Some(b.clone());
                self.backups.refresh(Some(path));
                self.backups.select(&b);
            }
            Err(e) => self.error("Backup failed", e),
        }
    }

    /// Copies `backup` over the save it was made from (backing that file up
    /// first) and opens the result.
    pub(in crate::app) fn restore_backup(&mut self, backup: &Path) {
        let Some(target) = save::backup_source(backup).or_else(|| self.backup_target()) else {
            return;
        };
        if let Err(e) = save::SaveFile::load(backup) {
            self.error("The backup cannot be read", e);
            return;
        }
        if target.exists()
            && let Err(e) = save::create_backup(&target)
        {
            self.error("Could not back up the current file", e);
            return;
        }
        match std::fs::copy(backup, &target) {
            Ok(_) => {
                self.load(&target, false);
                self.backups.refresh(Some(target));
                self.set_status(format!("Restored {}", backup.display()), Level::Success);
            }
            Err(e) => self.error("Restore failed", e),
        }
    }

    pub(in crate::app) fn backup_action(&mut self, action: ui::backups::BackupAction) {
        use ui::backups::BackupAction;
        match action {
            BackupAction::Create => self.manual_backup(),
            BackupAction::Restore(path) => self.request(Pending::RestoreBackup(path)),
            BackupAction::Open(path) => self.request(Pending::Open(path)),
            BackupAction::OpenFolder(dir) => {
                let _ = std::fs::create_dir_all(&dir);
                Self::open_folder(&dir);
            }
            BackupAction::Delete(paths) => {
                let mut deleted = 0;
                let mut errors = Vec::new();
                for path in &paths {
                    match save::delete_backup(path) {
                        Ok(()) => deleted += 1,
                        Err(e) => errors.push(e.to_string()),
                    }
                }
                let target = self.backups.target().map(Path::to_path_buf);
                self.backups.refresh(target);
                if errors.is_empty() {
                    self.set_status(format!("Deleted {deleted} backup(s)"), Level::Success);
                } else {
                    self.error(
                        &format!("Deleted {deleted} backup(s), could not delete the rest"),
                        errors.join("; "),
                    );
                }
            }
        }
    }
}
