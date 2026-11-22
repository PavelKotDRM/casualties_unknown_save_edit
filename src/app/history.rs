//! Bounded undo/redo history for the current save.

use serde_json::Value;

use super::EditorApp;

const MAX_UNDO: usize = 200;

pub(super) struct UndoStack<T> {
    committed: T,
    undo: Vec<T>,
    redo: Vec<T>,
}

impl<T> UndoStack<T> {
    pub(super) fn new(committed: T) -> Self {
        Self {
            committed,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    fn reset(&mut self, committed: T) {
        self.committed = committed;
        self.undo.clear();
        self.redo.clear();
    }

    pub(in crate::app) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(in crate::app) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

impl<T: Clone + PartialEq> UndoStack<T> {
    pub(in crate::app) fn has_uncommitted(&self, current: &T) -> bool {
        current != &self.committed
    }

    fn commit(&mut self, current: &T) -> bool {
        if !self.has_uncommitted(current) {
            return false;
        }
        self.undo.push(self.committed.clone());
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.committed = current.clone();
        self.redo.clear();
        true
    }

    fn undo(&mut self, current: &T) -> Option<T> {
        let previous = self.undo.pop()?;
        self.redo.push(current.clone());
        self.committed = previous.clone();
        Some(previous)
    }

    fn redo(&mut self, current: &T) -> Option<T> {
        let next = self.redo.pop()?;
        self.undo.push(current.clone());
        self.committed = next.clone();
        Some(next)
    }
}

impl EditorApp {
    /// Whether the current document differs from the file on disk.
    pub fn is_dirty(&self) -> bool {
        self.save
            .as_ref()
            .is_some_and(|save| save.data != self.clean)
    }

    pub(in crate::app) fn reset_history(&mut self) {
        let data = self
            .save
            .as_ref()
            .map_or(Value::Null, |save| save.data.clone());
        self.clean = data.clone();
        self.history.reset(data);
        self.raw.text.clear();
        self.raw.text_dirty = false;
        self.inventory.selected = None;
    }

    /// Records the current document in the undo history if it changed.
    pub(in crate::app) fn commit(&mut self) {
        let Some(data) = self.save.as_ref().map(|save| save.data.clone()) else {
            return;
        };
        if self.history.commit(&data) && !self.raw.text_dirty {
            self.raw.text.clear();
        }
    }

    pub(in crate::app) fn undo(&mut self) {
        self.commit();
        let Some(save) = self.save.as_mut() else {
            return;
        };
        let Some(previous) = self.history.undo(&save.data) else {
            return;
        };
        save.data = previous;
        self.raw.text.clear();
    }

    pub(in crate::app) fn redo(&mut self) {
        let Some(save) = self.save.as_mut() else {
            return;
        };
        let Some(next) = self.history.redo(&save.data) else {
            return;
        };
        save.data = next;
        self.raw.text.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::UndoStack;
    use serde_json::json;

    fn app() -> EditorApp {
        EditorApp {
            save: None,
            clean: Value::Null,
            history: UndoStack::new(Value::Null),
            tab: Tab::Overview,
            status: None,
            last_backup: None,
            config: Config {
                auto_refresh: false,
                ..Default::default()
            },
            languages: Vec::new(),
            interface_packs: Vec::new(),
            locale: None,
            item_stats: HashMap::new(),
            item_effects: Default::default(),
            crafting: Vec::new(),
            crafting_icons: HashMap::new(),
            crafting_icons_error: None,
            drop_sources: Vec::new(),
            game_info: None,
            limbs: Default::default(),
            inventory: Default::default(),
            raw: Default::default(),
            backups: Default::default(),
            about_open: false,
            pending: None,
            new_save_wizard: None,
            progress: GameProgress::unknown("Test game data unavailable".into()),
            achievements: None,
            achievements_stamp: None,
            achievements_ui: Default::default(),
            last_refresh: Instant::now(),
            game_stamp: Vec::new(),
            disk_seen: None,
            wiki: Default::default(),
            allow_close: false,
            title: String::new(),
            ctx: egui::Context::default(),
            cjk_font: false,
            error_log: Default::default(),
            item_sync_error: None,
            crash: None,
            report: Default::default(),
        }
    }

    #[test]
    #[allow(deprecated)]
    fn opening_and_reloading_are_clean_but_real_edits_need_confirmation() {
        for format in [SaveFormat::Gzip, SaveFormat::PlainJson] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("first.sv");
            let document = json!({"body": {}, "limbs": [], "items": [], "runTime": 12.5});
            let bytes = save::encode(&document, format);
            std::fs::write(&path, &bytes).unwrap();
            let mut app = app();
            app.open(&path);
            assert!(
                !app.is_dirty(),
                "opening a save must establish its clean baseline"
            );
            assert!(!app.history.can_undo());
            assert!(!app.history.can_redo());

            let context = app.ctx.clone();
            let mut output = context.run_ui(Default::default(), |ui| app.central(ui));
            output.textures_delta.clear();
            app.commit();
            assert!(!app.is_dirty());
            assert!(!app.history.can_undo());
            assert_eq!(app.save.as_ref().unwrap().data, document);
            assert_eq!(std::fs::read(&path).unwrap(), bytes);

            app.save.as_mut().unwrap().data["runTime"] = json!(20.0);
            app.commit();
            assert!(app.is_dirty());
            app.request(Pending::Close);
            assert!(matches!(app.pending, Some(Pending::Close)));
            assert!(!app.allow_close);
            app.pending = None;
            app.undo();
            assert!(
                !app.is_dirty(),
                "undoing to the loaded document must be clean"
            );
            app.redo();
            assert!(app.is_dirty());

            app.perform(Pending::Reload);
            assert!(!app.is_dirty());
            assert!(!app.history.can_undo());
            assert!(!app.history.can_redo());
            assert_eq!(app.save.as_ref().unwrap().data, document);

            let other = directory.path().join("second.sv");
            let other_document = json!({"body": {}, "limbs": [], "items": [], "runTime": 99.0});
            std::fs::write(&other, save::encode(&other_document, format)).unwrap();
            app.request(Pending::Open(other.clone()));
            assert!(
                app.pending.is_none(),
                "switching an unedited save needs no prompt"
            );
            assert_eq!(app.save.as_ref().unwrap().path, other);
            assert_eq!(app.save.as_ref().unwrap().data, other_document);
            assert!(!app.is_dirty());
            app.request(Pending::Close);
            assert!(app.pending.is_none());
            assert!(app.allow_close);
        }
    }

    #[test]
    fn edits_are_grouped_and_new_commits_discard_redo() {
        let mut history = UndoStack::new(0);
        assert!(history.commit(&1));
        assert!(history.commit(&2));
        assert_eq!(history.undo(&2), Some(1));
        assert_eq!(history.redo(&1), Some(2));
        assert!(history.commit(&3));
        assert!(!history.can_redo());
        assert_eq!(history.undo(&3), Some(2));
    }

    #[test]
    fn restore_waits_for_unsaved_changes_and_preserves_them_on_error() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("save.sv");
        let document = json!({"body": {}, "limbs": [], "items": [], "runTime": 12});
        std::fs::write(&path, save::encode(&document, SaveFormat::Gzip)).unwrap();
        let backup = save::create_backup(&path).unwrap();
        let mut app = app();
        app.load(&path, false);
        app.save.as_mut().unwrap().data["runTime"] = json!(20);
        app.backup_action(ui::backups::BackupAction::Restore {
            backup: backup.clone(),
            target: path.clone(),
        });
        assert!(matches!(app.pending, Some(Pending::RestoreBackup { .. })));
        assert_eq!(app.save.as_ref().unwrap().data["runTime"], json!(20));

        std::fs::write(&backup, b"corrupted").unwrap();
        let pending = app.pending.take().unwrap();
        app.perform(pending);
        assert!(app.is_dirty());
        assert_eq!(app.save.as_ref().unwrap().data["runTime"], json!(20));
        assert_eq!(save::SaveFile::load(&path).unwrap().data, document);
    }

    #[test]
    fn restore_refuses_unapplied_raw_text() {
        let mut app = app();
        app.raw.text_dirty = true;
        app.backup_action(ui::backups::BackupAction::Restore {
            backup: PathBuf::from("backup"),
            target: PathBuf::from("save"),
        });
        assert!(app.pending.is_none());
        assert!(matches!(app.status, Some((_, Level::Error, _))));
        assert!(app.raw.text_dirty);
    }

    #[test]
    fn successful_restore_opens_a_clean_document_and_resets_history() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("save.sv");
        let document = json!({"body": {}, "limbs": [], "items": [], "runTime": 12});
        std::fs::write(&path, save::encode(&document, SaveFormat::Gzip)).unwrap();
        let backup = save::create_backup(&path).unwrap();
        let mut app = app();
        app.load(&path, false);
        app.save.as_mut().unwrap().data["runTime"] = json!(20);
        app.commit();

        app.restore_backup(&backup, &path);
        assert_eq!(app.save.as_ref().unwrap().data, document);
        assert!(!app.is_dirty());
        assert!(!app.history.can_undo());
        assert!(app.last_backup.as_ref().unwrap().is_file());
    }

    #[test]
    fn undo_history_is_bounded() {
        let mut history = UndoStack::new(0);
        for value in 1..=201 {
            history.commit(&value);
        }

        assert_eq!(history.undo(&201), Some(200));
        for value in (1..=199).rev() {
            assert_eq!(history.undo(&(value + 1)), Some(value));
        }
        assert!(!history.can_undo());
    }
}
