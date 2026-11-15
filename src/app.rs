//! The egui application: menus, tabs, undo/redo, backups and file handling.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Key, KeyboardShortcut, Modifiers, RichText, Ui};

mod view;
use serde_json::{Value, json};

use crate::game_data::{self, ItemStats};
use crate::interface_packs::{self, InterfacePack};
use crate::locale::{self, LanguageFile, Locale};
use crate::save::{self, BackupInfo, SaveError, SaveFile, SaveFormat};
use crate::ui::{self, InterfaceLanguage, Names, colors};

const MAX_UNDO: usize = 200;
const CTRL_S: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const CTRL_O: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const CTRL_Z: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const CTRL_Y: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const CTRL_SHIFT_Z: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);

/// Tabs of the main window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// Dashboard.
    Overview,
    /// Identity and skills.
    Character,
    /// Body values.
    Health,
    /// Limbs with the body figure.
    Limbs,
    /// Items.
    Inventory,
    /// Run progress and settings.
    Run,
    /// Raw JSON tree / text.
    Raw,
}

impl Tab {
    /// All tabs in display order.
    pub const ALL: [Tab; 7] = [
        Tab::Overview,
        Tab::Character,
        Tab::Health,
        Tab::Limbs,
        Tab::Inventory,
        Tab::Run,
        Tab::Raw,
    ];

    /// Tab title with icon.
    pub fn title(self, language: InterfaceLanguage) -> String {
        let (icon, label) = match self {
            Tab::Overview => ("🏠", "Overview"),
            Tab::Character => ("🐾", "Character"),
            Tab::Health => ("❤", "Health"),
            Tab::Limbs => ("🦴", "Limbs"),
            Tab::Inventory => ("🎒", "Inventory"),
            Tab::Run => ("🗺", "Run"),
            Tab::Raw => ("{ }", "Raw JSON"),
        };
        format!("{icon} {}", language.text(label))
    }
}

/// Actions that would discard unsaved changes and need confirmation.
#[derive(Debug, Clone)]
enum Pending {
    NewSave,
    Open(PathBuf),
    OpenDialog,
    Reload,
    RestoreBackup(PathBuf),
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NewSaveStep {
    World,
    Character,
}

struct NewSaveWizard {
    data: Value,
    step: NewSaveStep,
    preset: String,
}

/// Severity of a status-bar message.
#[derive(Clone, Copy, PartialEq)]
enum Level {
    Info,
    Success,
    Error,
}

/// Persistent user settings (`%APPDATA%\cu_save_editor\config.json`).
struct Config {
    game_dir: Option<PathBuf>,
    language: Option<String>,
    interface_language: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            game_dir: None,
            language: None,
            interface_language: "en".to_owned(),
        }
    }
}

impl Config {
    fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("cu_save_editor").join("config.json"))
    }

    fn load() -> Self {
        let Some(v) = Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        else {
            return Self::default();
        };
        Self {
            game_dir: v.get("gameDir").and_then(Value::as_str).map(PathBuf::from),
            language: v.get("language").and_then(Value::as_str).map(str::to_owned),
            interface_language: v
                .get("interfaceLanguage")
                .and_then(Value::as_str)
                .unwrap_or("en")
                .to_owned(),
        }
    }

    fn store(&self) -> Result<(), String> {
        let path = Self::path().ok_or("Cannot determine the editor settings folder")?;
        let v = json!({
            "gameDir": self.game_dir.as_ref().map(|p| p.to_string_lossy()),
            "language": self.language,
            "interfaceLanguage": self.interface_language,
        });
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&path, save::to_pretty_json(&v))
            .map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Adds a Windows system font as a fallback for glyphs missing from egui's
/// built-in fonts (symbols, CJK item names). Missing files are ignored.
fn add_fallback_font(ctx: &egui::Context, name: &str, file: &str) -> bool {
    let Some(dir) = std::env::var_os("WINDIR").map(PathBuf::from) else {
        return false;
    };
    let Ok(bytes) = std::fs::read(dir.join("Fonts").join(file)) else {
        return false;
    };
    let families = [egui::FontFamily::Proportional, egui::FontFamily::Monospace]
        .into_iter()
        .map(|family| egui::epaint::text::InsertFontFamily {
            family,
            priority: egui::epaint::text::FontPriority::Lowest,
        })
        .collect();
    ctx.add_font(egui::epaint::text::FontInsert::new(
        name,
        egui::FontData::from_owned(bytes),
        families,
    ));
    true
}

/// The save editor application.
pub struct EditorApp {
    save: Option<SaveFile>,
    /// Document as it is on disk (for the "modified" indicator).
    clean: Value,
    /// Last state pushed to the undo history.
    committed: Value,
    undo: Vec<Value>,
    redo: Vec<Value>,
    tab: Tab,
    status: Option<(String, Level, Instant)>,
    last_backup: Option<PathBuf>,
    config: Config,
    languages: Vec<LanguageFile>,
    interface_packs: Vec<Arc<InterfacePack>>,
    locale: Option<Locale>,
    item_stats: HashMap<String, ItemStats>,
    limbs: ui::limbs::LimbsState,
    inventory: ui::inventory::InventoryState,
    raw: ui::raw::TreeState,
    backups_open: bool,
    backups: Vec<BackupInfo>,
    about_open: bool,
    pending: Option<Pending>,
    new_save_wizard: Option<NewSaveWizard>,
    allow_close: bool,
    title: String,
    ctx: egui::Context,
    cjk_font: bool,
}

impl EditorApp {
    /// Creates the app and opens the default save if it exists.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.global_style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 5.0);
            style.spacing.button_padding = egui::vec2(8.0, 3.0);
        });
        let mut app = Self {
            save: None,
            clean: Value::Null,
            committed: Value::Null,
            undo: Vec::new(),
            redo: Vec::new(),
            tab: Tab::Overview,
            status: None,
            last_backup: None,
            config: Config::load(),
            languages: Vec::new(),
            interface_packs: Vec::new(),
            locale: None,
            item_stats: HashMap::new(),
            limbs: Default::default(),
            inventory: Default::default(),
            raw: Default::default(),
            backups_open: false,
            backups: Vec::new(),
            about_open: false,
            pending: None,
            new_save_wizard: None,
            allow_close: false,
            title: String::new(),
            ctx: cc.egui_ctx.clone(),
            cjk_font: false,
        };
        add_fallback_font(&cc.egui_ctx, "segoe_symbol", "seguisym.ttf");
        app.init_game_data();
        let startup = std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .or_else(save::default_save_path);
        match startup {
            Some(path) if path.exists() => app.open(&path),
            Some(path) => app.set_status(
                format!(
                    "No save found at {} — start a run and use Save & Quit, or open a file.",
                    path.display()
                ),
                Level::Info,
            ),
            None => {}
        }
        app.reload_interface_packs();
        app
    }

    // ----- game data --------------------------------------------------

    fn selected_interface_pack(&self) -> Option<Arc<InterfacePack>> {
        self.interface_packs
            .iter()
            .find(|pack| pack.code == self.config.interface_language)
            .cloned()
    }

    fn reload_interface_packs(&mut self) {
        let Some(directory) = interface_packs::directory() else {
            self.set_status(
                "Cannot determine the interface language folder",
                Level::Error,
            );
            return;
        };
        let (packs, errors) = interface_packs::load(&directory);
        self.interface_packs = packs;
        if self.selected_interface_pack().is_some() && !self.cjk_font {
            self.cjk_font = add_fallback_font(&self.ctx, "cjk", "msyh.ttc")
                || add_fallback_font(&self.ctx, "cjk", "simsun.ttc");
        }
        if !errors.is_empty() {
            let message = errors.join("; ");
            eprintln!("Interface language packs: {message}");
            self.set_status(format!("Interface language packs: {message}"), Level::Error);
        } else if self.config.interface_language != "en"
            && self.config.interface_language != "ru"
            && self.selected_interface_pack().is_none()
        {
            self.set_status(
                format!(
                    "Interface language '{}' not found; using English",
                    self.config.interface_language
                ),
                Level::Error,
            );
        }
    }

    fn init_game_data(&mut self) {
        let dir = self
            .config
            .game_dir
            .clone()
            .filter(|d| locale::is_game_dir(d))
            .or_else(|| locale::detect_game_dir(save::default_save_dir().as_deref()));
        if let Err(error) = self.set_game_dir(dir) {
            self.error("Could not sync item stats", error);
        }
    }

    fn set_game_dir(&mut self, dir: Option<PathBuf>) -> Result<(), String> {
        self.languages = dir
            .as_deref()
            .map(locale::list_languages)
            .unwrap_or_default();
        let stats = dir.as_deref().map(game_data::load_item_stats).transpose();
        self.item_stats = stats
            .as_ref()
            .ok()
            .and_then(Clone::clone)
            .unwrap_or_default();
        self.config.game_dir = dir;
        let wanted = self.config.language.clone().unwrap_or_else(|| "EN".into());
        let lang = self
            .languages
            .iter()
            .find(|l| l.code == wanted)
            .or_else(|| self.languages.iter().find(|l| l.code == "EN"))
            .or(self.languages.first())
            .cloned();
        self.set_language(lang);
        stats.map(|_| ())
    }

    fn set_language(&mut self, lang: Option<LanguageFile>) {
        self.locale = lang.as_ref().and_then(|l| Locale::load(&l.path));
        if let Some(l) = lang {
            if l.code.starts_with("zh") && !self.cjk_font {
                self.cjk_font = add_fallback_font(&self.ctx, "cjk", "msyh.ttc")
                    || add_fallback_font(&self.ctx, "cjk", "simsun.ttc");
            }
            self.config.language = Some(l.code);
        }
        if let Err(error) = self.config.store() {
            self.error("Could not save editor settings", error);
        }
    }

    fn set_interface_language(&mut self, code: &str) {
        self.config.interface_language = code.to_owned();
        if code != "en" && code != "ru" && !self.cjk_font {
            self.cjk_font = add_fallback_font(&self.ctx, "cjk", "msyh.ttc")
                || add_fallback_font(&self.ctx, "cjk", "simsun.ttc");
        }
        if let Err(error) = self.config.store() {
            self.error("Could not save editor settings", error);
        }
    }

    // ----- status -----------------------------------------------------

    fn set_status(&mut self, text: impl Into<String>, level: Level) {
        self.status = Some((text.into(), level, Instant::now()));
    }

    fn error(&mut self, context: &str, e: impl std::fmt::Display) {
        self.set_status(format!("{context}: {e}"), Level::Error);
    }

    // ----- document state ---------------------------------------------

    /// Whether the document differs from the file on disk.
    pub fn is_dirty(&self) -> bool {
        self.save.as_ref().is_some_and(|s| s.data != self.clean)
    }

    fn reset_history(&mut self) {
        let data = self.save.as_ref().map_or(Value::Null, |s| s.data.clone());
        self.clean = data.clone();
        self.committed = data;
        self.undo.clear();
        self.redo.clear();
        self.raw.text.clear();
        self.raw.text_dirty = false;
        self.inventory.selected = None;
    }

    /// Records the current document in the undo history if it changed.
    fn commit(&mut self) {
        let Some(save) = &self.save else { return };
        if save.data != self.committed {
            let previous = std::mem::replace(&mut self.committed, save.data.clone());
            self.undo.push(previous);
            if self.undo.len() > MAX_UNDO {
                self.undo.remove(0);
            }
            self.redo.clear();
            if !self.raw.text_dirty {
                self.raw.text.clear();
            }
        }
    }

    fn undo(&mut self) {
        self.commit();
        let (Some(save), Some(prev)) = (self.save.as_mut(), self.undo.pop()) else {
            return;
        };
        let current = std::mem::replace(&mut save.data, prev);
        self.redo.push(current);
        self.committed = save.data.clone();
        self.raw.text.clear();
    }

    fn redo(&mut self) {
        let (Some(save), Some(next)) = (self.save.as_mut(), self.redo.pop()) else {
            return;
        };
        let current = std::mem::replace(&mut save.data, next);
        self.undo.push(current);
        self.committed = save.data.clone();
        self.raw.text.clear();
    }

    // ----- file operations --------------------------------------------

    fn request(&mut self, action: Pending) {
        if self.is_dirty() {
            self.pending = Some(action);
        } else {
            self.perform(action);
        }
    }

    fn perform(&mut self, action: Pending) {
        match action {
            Pending::NewSave => {
                self.new_save_wizard = Some(NewSaveWizard {
                    data: save::new_game_data(),
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
    fn open(&mut self, path: &Path) {
        self.load(path, true);
    }

    fn load(&mut self, path: &Path, backup: bool) {
        match SaveFile::load(path) {
            Ok(file) => {
                let in_backup_dir = path
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|n| n == save::BACKUP_DIR_NAME);
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

    fn open_dialog(&mut self) {
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

    fn current_dir(&self) -> Option<PathBuf> {
        self.save
            .as_ref()
            .and_then(|s| s.path.parent().map(Path::to_path_buf))
            .or_else(save::default_save_dir)
            .filter(|d| d.is_dir())
    }

    fn save(&mut self) {
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

    fn save_as(&mut self) {
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

    fn create_new_save(&mut self, data: Value) -> bool {
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

    fn export_json(&mut self) {
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

    fn import_json(&mut self) {
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

    fn manual_backup(&mut self) {
        let Some(path) = self.save.as_ref().map(|s| s.path.clone()) else {
            return;
        };
        match save::create_backup(&path) {
            Ok(b) => {
                self.set_status(format!("Backup created: {}", b.display()), Level::Success);
                self.last_backup = Some(b);
                self.backups = save::list_backups(&path);
            }
            Err(e) => self.error("Backup failed", e),
        }
    }

    fn restore_backup(&mut self, backup: &Path) {
        let Some(target) = self.save.as_ref().map(|s| s.path.clone()) else {
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
                self.backups = save::list_backups(&target);
                self.set_status(format!("Restored {}", backup.display()), Level::Success);
            }
            Err(e) => self.error("Restore failed", e),
        }
    }

    fn open_folder(path: &Path) {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }

    // ----- UI ---------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let text_focused = ctx.memory(|m| m.focused().is_some());
        if ctx.input_mut(|i| i.consume_shortcut(&CTRL_S)) && self.save.is_some() {
            self.save();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&CTRL_O)) {
            self.request(Pending::OpenDialog);
        }
        if text_focused {
            return; // let text fields handle their own undo
        }
        if ctx.input_mut(|i| i.consume_shortcut(&CTRL_SHIFT_Z) || i.consume_shortcut(&CTRL_Y)) {
            self.redo();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&CTRL_Z)) {
            self.undo();
        }
    }
}

impl eframe::App for EditorApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) && self.is_dirty() && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }

        self.shortcuts(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let title = match &self.save {
            Some(s) => format!(
                "{}{} — CU Save Editor",
                if self.is_dirty() { "● " } else { "" },
                s.path
                    .file_name()
                    .map_or(String::new(), |n| n.to_string_lossy().into_owned())
            ),
            None => "CU Save Editor".to_owned(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        if self.save.is_some() {
            egui::Panel::top("tabs").show(ui, |ui| {
                ui.add_space(3.0);
                self.tab_bar(ui);
                ui.add_space(1.0);
            });
        }
        egui::CentralPanel::default().show(ui, |ui| {
            if self.save.is_some() {
                self.central(ui);
            } else {
                self.welcome(ui);
            }
        });
        self.dialogs(&ctx);

        // Group continuous edits (drags, typing) into one undo step.
        let busy = ctx.input(|i| i.pointer.any_down()) || ctx.memory(|m| m.focused().is_some());
        if !busy {
            self.commit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_interface_language_is_english() {
        assert_eq!(Config::default().interface_language, "en");
    }
}
