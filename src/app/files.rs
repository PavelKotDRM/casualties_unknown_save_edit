//! Application files operations.

use super::*;

impl EditorApp {
    // ----- file operations --------------------------------------------

    pub(in crate::app) fn request(&mut self, action: Pending) {
        if self.is_dirty() {
            self.pending = Some(action);
        } else {
            self.perform(action);
        }
    }

    pub(in crate::app) fn perform(&mut self, action: Pending) {
        match action {
            Pending::NewSave => {
                if !self.game_ready() {
                    self.error(
                        "Cannot create a save",
                        "Game data unavailable. Select the game folder in Game data.",
                    );
                    return;
                }
                let data = match save::try_new_game_data() {
                    Ok(data) => data,
                    Err(error) => {
                        self.error("Cannot create a save", error);
                        return;
                    }
                };
                self.refresh_progress();
                self.new_save_wizard = Some(NewSaveWizard {
                    data,
                    step: NewSaveStep::World,
                    preset: "normal".to_owned(),
                });
            }
            Pending::Open(path) => self.open(&path),
            Pending::OpenDialog => self.open_dialog(),
            Pending::Reload => {
                if let Some(path) = self.save.as_ref().map(|s| s.path.clone()) {
                    self.load(&path, false);
                }
            }
            Pending::RestoreBackup(path) => self.restore_backup(&path),
            Pending::Close => self.allow_close = true,
        }
    }

    /// Opens a save and creates a backup of it (unless it is a backup itself).
    pub(in crate::app) fn open(&mut self, path: &Path) {
        self.load(path, true);
    }

    pub(in crate::app) fn load(&mut self, path: &Path, backup: bool) {
        match SaveFile::load(path) {
            Ok(file) => {
                let in_backup_dir = save::is_backup(path);
                let mut msg = format!("Opened {}", path.display());
                if backup && !in_backup_dir {
                    match save::create_backup(path) {
                        Ok(b) => {
                            msg.push_str(&format!(" · backup: {}", b.display()));
                            self.last_backup = Some(b);
                        }
                        Err(e) => {
                            self.save = Some(file);
                            self.reset_history();
                            self.error("Opened, but the backup failed", e);
                            return;
                        }
                    }
                }
                self.save = Some(file);
                self.reset_history();
                self.set_status(msg, Level::Success);
            }
            Err(e) => self.error(&format!("Cannot open {}", path.display()), e),
        }
    }

    pub(in crate::app) fn open_dialog(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Open save")
            .add_filter("Save files", &["sv", "json"])
            .add_filter("All files", &["*"]);
        if let Some(dir) = self.current_dir() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.open(&path);
        }
    }

    pub(in crate::app) fn current_dir(&self) -> Option<PathBuf> {
        self.save
            .as_ref()
            .and_then(|s| s.path.parent().map(Path::to_path_buf))
            .or_else(save::default_save_dir)
            .filter(|d| d.is_dir())
    }

    pub(in crate::app) fn save(&mut self) {
        self.commit();
        let Some(file) = self.save.as_mut() else {
            return;
        };
        match file.write() {
            Ok(extra) => {
                self.clean = file.data.clone();
                let mut msg = format!("Saved {}", file.path.display());
                if let Some(b) = extra {
                    msg.push_str(&format!(
                        " · the file had changed on disk, previous version backed up to {}",
                        b.display()
                    ));
                    self.last_backup = Some(b);
                }
                self.set_status(msg, Level::Success);
            }
            Err(e) => self.error("Save failed", e),
        }
    }

    pub(in crate::app) fn save_as(&mut self) {
        self.commit();
        let Some(file) = self.save.as_ref() else {
            return;
        };
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save as")
            .set_file_name(
                file.path
                    .file_name()
                    .map_or("save.sv".into(), |n| n.to_string_lossy().into_owned()),
            )
            .add_filter("Game save (gzip)", &["sv"])
            .add_filter("Plain JSON", &["json"]);
        if let Some(dir) = self.current_dir() {
            dialog = dialog.set_directory(dir);
        }

        let Some(path) = dialog.save_file() else {
            return;
        };
        let format = if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        {
            SaveFormat::PlainJson
        } else {
            SaveFormat::Gzip
        };
        let file = self.save.as_mut().expect("checked above");
        match file.write_as(&path, format) {
            Ok(()) => {
                self.clean = file.data.clone();
                self.set_status(format!("Saved as {}", path.display()), Level::Success);
            }
            Err(e) => self.error("Save failed", e),
        }
    }

    pub(in crate::app) fn create_new_save(&mut self, data: Value) -> bool {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Create new save")
            .set_file_name(save::SAVE_FILE_NAME)
            .add_filter("Game save (gzip)", &["sv"])
            .add_filter("Plain JSON", &["json"]);
        if let Some(dir) = self.current_dir() {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.save_file() else {
            return false;
        };
        let format = if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            SaveFormat::PlainJson
        } else {
            SaveFormat::Gzip
        };
        let mut file = SaveFile {
            path: path.clone(),
            format,
            data,
            disk_bytes: Vec::new(),
        };
        match file.write() {
            Ok(backup) => {
                self.last_backup = backup;
                self.save = Some(file);
                self.reset_history();
                self.tab = Tab::Overview;
                self.set_status(format!("Created {}", path.display()), Level::Success);
                true
            }
            Err(error) => {
                self.error("Could not create save", error);
                false
            }
        }
    }

    pub(in crate::app) fn export_json(&mut self) {
        self.commit();
        let Some(file) = self.save.as_ref() else {
            return;
        };
        let stem = file
            .path
            .file_stem()
            .map_or("save".into(), |s| s.to_string_lossy().into_owned());
        let mut dialog = rfd::FileDialog::new()
            .set_title("Export readable JSON")
            .set_file_name(format!("{stem}.json"))
            .add_filter("JSON", &["json"]);
        if let Some(dir) = self.current_dir() {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.save_file() else {
            return;
        };
        match std::fs::write(&path, save::to_pretty_json(&file.data)) {
            Ok(()) => self.set_status(format!("Exported {}", path.display()), Level::Success),
            Err(e) => self.error("Export failed", e),
        }
    }

    pub(in crate::app) fn import_json(&mut self) {
        if self.save.is_none() {
            return;
        }
        let mut dialog = rfd::FileDialog::new()
            .set_title("Import JSON into the current save")
            .add_filter("JSON or save", &["json", "sv"]);
        if let Some(dir) = self.current_dir() {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.pick_file() else {
            return;
        };
        let result = std::fs::read(&path)
            .map_err(|e| SaveError::Io(path.clone(), e))
            .and_then(|b| save::decode(&b));
        match result {
            Ok((data, _)) => {
                self.commit();
                if let Some(file) = self.save.as_mut() {
                    file.data = data;
                }
                self.commit();
                self.set_status(
                    format!(
                        "Imported {} — review and save to apply (Ctrl+Z to undo)",
                        path.display()
                    ),
                    Level::Success,
                );
            }
            Err(e) => self.error("Import failed", e),
        }
    }
}
