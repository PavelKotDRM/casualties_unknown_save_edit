//! The egui application: menus, tabs, undo/redo, backups and file handling.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Key, KeyboardShortcut, Modifiers, RichText, Ui};
use serde_json::{Value, json};

use crate::locale::{self, LanguageFile, Locale};
use crate::save::{self, BackupInfo, SaveError, SaveFile, SaveFormat};
use crate::ui::{self, Names, colors};

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
    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "🏠 Overview",
            Tab::Character => "🐾 Character",
            Tab::Health => "❤ Health",
            Tab::Limbs => "🦴 Limbs",
            Tab::Inventory => "🎒 Inventory",
            Tab::Run => "🗺 Run",
            Tab::Raw => "{ } Raw JSON",
        }
    }
}

/// Actions that would discard unsaved changes and need confirmation.
#[derive(Debug, Clone)]
enum Pending {
    Open(PathBuf),
    OpenDialog,
    Reload,
    RestoreBackup(PathBuf),
    Close,
}

/// Severity of a status-bar message.
#[derive(Clone, Copy, PartialEq)]
enum Level {
    Info,
    Success,
    Error,
}

/// Persistent user settings (`%APPDATA%\cu_save_editor\config.json`).
#[derive(Default)]
struct Config {
    game_dir: Option<PathBuf>,
    language: Option<String>,
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
        }
    }

    fn store(&self) {
        let Some(path) = Self::path() else { return };
        let v = json!({
            "gameDir": self.game_dir.as_ref().map(|p| p.to_string_lossy()),
            "language": self.language,
        });
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, save::to_pretty_json(&v));
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
    locale: Option<Locale>,
    limbs: ui::limbs::LimbsState,
    inventory: ui::inventory::InventoryState,
    raw: ui::raw::TreeState,
    backups_open: bool,
    backups: Vec<BackupInfo>,
    about_open: bool,
    pending: Option<Pending>,
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
            locale: None,
            limbs: Default::default(),
            inventory: Default::default(),
            raw: Default::default(),
            backups_open: false,
            backups: Vec::new(),
            about_open: false,
            pending: None,
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
        app
    }

    // ----- game data --------------------------------------------------

    fn init_game_data(&mut self) {
        let dir = self
            .config
            .game_dir
            .clone()
            .filter(|d| locale::is_game_dir(d))
            .or_else(|| locale::detect_game_dir(save::default_save_dir().as_deref()));
        self.set_game_dir(dir);
    }

    fn set_game_dir(&mut self, dir: Option<PathBuf>) {
        self.languages = dir
            .as_deref()
            .map(locale::list_languages)
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
        self.config.store();
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

    fn menu_bar(&mut self, ui: &mut Ui) {
        let loaded = self.save.is_some();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("📂 Open game save").clicked() {
                    if let Some(p) = save::default_save_path() {
                        self.request(Pending::Open(p));
                    }
                    ui.close();
                }
                if ui
                    .add(egui::Button::new("📂 Open…").shortcut_text("Ctrl+O"))
                    .clicked()
                {
                    self.request(Pending::OpenDialog);
                    ui.close();
                }
                if ui
                    .add_enabled(loaded, egui::Button::new("⟲ Reload from disk"))
                    .clicked()
                {
                    self.request(Pending::Reload);
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(loaded, egui::Button::new("💾 Save").shortcut_text("Ctrl+S"))
                    .clicked()
                {
                    self.save();
                    ui.close();
                }
                if ui
                    .add_enabled(loaded, egui::Button::new("💾 Save as…"))
                    .clicked()
                {
                    self.save_as();
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(loaded, egui::Button::new("📤 Export readable JSON…"))
                    .clicked()
                {
                    self.export_json();
                    ui.close();
                }
                if ui
                    .add_enabled(loaded, egui::Button::new("📥 Import JSON…"))
                    .clicked()
                {
                    self.import_json();
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(loaded, egui::Button::new("🗄 Create backup now"))
                    .clicked()
                {
                    self.manual_backup();
                    ui.close();
                }
                if ui
                    .add_enabled(loaded, egui::Button::new("🗄 Backups…"))
                    .clicked()
                {
                    self.backups_open = true;
                    if let Some(s) = &self.save {
                        self.backups = save::list_backups(&s.path);
                    }
                    ui.close();
                }
                if ui.button("📁 Open save folder").clicked() {
                    if let Some(d) = self.current_dir() {
                        Self::open_folder(&d);
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button("🚪 Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    ui.close();
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui
                    .add_enabled(
                        !self.undo.is_empty() || self.is_uncommitted(),
                        egui::Button::new("↶ Undo").shortcut_text("Ctrl+Z"),
                    )
                    .clicked()
                {
                    self.undo();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        !self.redo.is_empty(),
                        egui::Button::new("↷ Redo").shortcut_text("Ctrl+Y"),
                    )
                    .clicked()
                {
                    self.redo();
                    ui.close();
                }
                if ui
                    .add_enabled(self.is_dirty(), egui::Button::new("⟲ Revert all changes"))
                    .clicked()
                {
                    self.commit();
                    if let Some(s) = self.save.as_mut() {
                        s.data = self.clean.clone();
                    }
                    self.commit();
                    ui.close();
                }
                ui.separator();
                ui.add_enabled_ui(loaded, |ui| {
                    if let Some(data) = self.save.as_mut().map(|s| &mut s.data) {
                        let mut fix = |ui: &mut Ui, label: &str, f: &dyn Fn(&mut Value)| {
                            if ui.button(label).clicked() {
                                f(data);
                                ui.close();
                            }
                        };
                        fix(ui, "✚ Heal all limbs", &|d| {
                            crate::model::heal_limbs(d, false)
                        });
                        fix(ui, "✚ Heal + regrow limbs", &|d| {
                            crate::model::heal_limbs(d, true)
                        });
                        fix(ui, "❤ Restore vitals", &crate::model::restore_vitals);
                        fix(ui, "🍖 Fill needs", &crate::model::fill_needs);
                    }
                });
            });
            ui.menu_button("View", |ui| {
                for tab in Tab::ALL {
                    if ui.selectable_label(self.tab == tab, tab.title()).clicked() {
                        self.tab = tab;
                        ui.close();
                    }
                }
                ui.separator();
                egui::widgets::global_theme_preference_buttons(ui);
            });
            ui.menu_button("Game data", |ui| {
                match &self.config.game_dir {
                    Some(d) => ui.weak(format!("Game folder: {}", d.display())),
                    None => ui.colored_label(colors::WARN, "Game folder not found — showing ids"),
                };
                ui.separator();
                ui.label("Language of names:");
                let langs = self.languages.clone();
                let current = self.config.language.clone();
                for l in langs {
                    if ui
                        .selectable_label(current.as_deref() == Some(&l.code), &l.code)
                        .clicked()
                    {
                        self.set_language(Some(l));
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("📁 Set game folder…").clicked() {
                    if let Some(dir) = rfd::FileDialog::new()
                        .set_title("Casualties Unknown installation folder")
                        .pick_folder()
                    {
                        if locale::is_game_dir(&dir) {
                            self.set_game_dir(Some(dir));
                            self.set_status("Game data loaded", Level::Success);
                        } else {
                            self.set_status(
                                "That folder does not contain CasualtiesUnknown_Data\\Lang",
                                Level::Error,
                            );
                        }
                    }
                    ui.close();
                }
                if ui.button("🔍 Detect automatically").clicked() {
                    let dir = locale::detect_game_dir(save::default_save_dir().as_deref());
                    let found = dir.is_some();
                    self.set_game_dir(dir);
                    self.set_status(
                        if found {
                            "Game folder detected"
                        } else {
                            "Game folder not found"
                        },
                        if found { Level::Success } else { Level::Error },
                    );
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("ℹ About").clicked() {
                    self.about_open = true;
                    ui.close();
                }
            });
        });
    }

    fn is_uncommitted(&self) -> bool {
        self.save.as_ref().is_some_and(|s| s.data != self.committed)
    }

    fn tab_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                let selected = self.tab == tab;
                let text = RichText::new(tab.title()).size(15.0);
                if ui.add(egui::Button::selectable(selected, text)).clicked() && !selected {
                    self.tab = tab;
                    if tab == Tab::Raw && !self.raw.text_dirty {
                        self.raw.text.clear();
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let dirty = self.is_dirty();
                if ui
                    .add_enabled(dirty, egui::Button::new(RichText::new("💾 Save").strong()))
                    .on_hover_text("Write the save (Ctrl+S)")
                    .clicked()
                {
                    self.save();
                }
                if ui
                    .add_enabled(!self.redo.is_empty(), egui::Button::new("↷"))
                    .on_hover_text("Redo (Ctrl+Y)")
                    .clicked()
                {
                    self.redo();
                }
                if ui
                    .add_enabled(
                        !self.undo.is_empty() || self.is_uncommitted(),
                        egui::Button::new("↶"),
                    )
                    .on_hover_text("Undo (Ctrl+Z)")
                    .clicked()
                {
                    self.undo();
                }
            });
        });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if let Some(file) = &self.save {
                let dirty = file.data != self.clean;
                if dirty {
                    ui.colored_label(colors::WARN, "● modified");
                } else {
                    ui.colored_label(colors::GOOD, "✔ saved");
                }
                ui.separator();
                ui.label(RichText::new(file.path.display().to_string()).monospace())
                    .on_hover_text("Current file");
                ui.weak(match file.format {
                    SaveFormat::Gzip => "gzip",
                    SaveFormat::PlainJson => "plain JSON",
                });
                if let Some(b) = &self.last_backup {
                    ui.separator();
                    ui.weak("backup:").on_hover_text(b.display().to_string());
                    ui.weak(
                        b.file_name()
                            .map_or(String::new(), |n| n.to_string_lossy().into_owned()),
                    );
                }
            }
            if let Some((text, level, at)) = &self.status {
                let color = match level {
                    Level::Info => ui.visuals().text_color(),
                    Level::Success => colors::GOOD,
                    Level::Error => colors::BAD,
                };
                if *level == Level::Error || at.elapsed() < Duration::from_secs(12) {
                    ui.separator();
                    ui.colored_label(color, text).on_hover_text(text);
                }
            }
        });
    }

    fn welcome(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.2);
            ui.label(RichText::new("Casualties Unknown — Save Editor").size(28.0).strong());
            ui.add_space(10.0);
            match save::default_save_path() {
                Some(p) if p.exists() => {
                    ui.label(format!("Save found: {}", p.display()));
                    if ui.button(RichText::new("📂 Open game save").size(18.0)).clicked() {
                        self.open(&p);
                    }
                }
                Some(p) => {
                    ui.label(format!("No save at {}", p.display()));
                    ui.weak("The game writes save.sv on \"Save & Quit\" and deletes it when the run is loaded again.");
                }
                None => {
                    ui.label("Cannot determine the save folder.");
                }
            }
            ui.add_space(6.0);
            if ui.button("📂 Open another file…").clicked() {
                self.open_dialog();
            }
            ui.add_space(6.0);
            ui.weak("A timestamped backup is created in SaveEditorBackups every time a save is opened.");
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(pending) = self.pending.clone() {
            let mut choice = None;
            egui::Modal::new(egui::Id::unique("confirm_discard")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.heading("Unsaved changes");
                ui.label(match &pending {
                    Pending::Close => "Save your changes before closing?",
                    Pending::RestoreBackup(_) => {
                        "Restoring a backup discards your unsaved changes. Save them first?"
                    }
                    _ => "Save your changes before continuing?",
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("💾 Save").clicked() {
                        choice = Some(true);
                    }
                    if ui
                        .button(RichText::new("Discard").color(colors::BAD))
                        .clicked()
                    {
                        choice = Some(false);
                    }
                    if ui.button("Cancel").clicked() {
                        self.pending = None;
                    }
                });
            });
            if let Some(save_first) = choice {
                self.pending = None;
                if save_first {
                    self.save();
                    if self.is_dirty() {
                        return; // saving failed; keep the editor open
                    }
                }
                if let Some(s) = self.save.as_mut() {
                    s.data = self.clean.clone();
                }
                let close = matches!(pending, Pending::Close);
                self.perform(pending);
                if close {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        if self.backups_open {
            let mut open = true;
            let mut restore = None;
            let mut open_backup = None;
            egui::Window::new("🗄 Backups")
                .open(&mut open)
                .default_size([560.0, 360.0])
                .show(ctx, |ui| {
                    let Some(path) = self.save.as_ref().map(|s| s.path.clone()) else { return };
                    let dir = save::backup_dir(&path);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(dir.display().to_string()).monospace());
                        if ui.button("📁 Open folder").clicked() {
                            let _ = std::fs::create_dir_all(&dir);
                            Self::open_folder(&dir);
                        }
                        if ui.button("➕ Backup now").clicked() {
                            self.manual_backup();
                        }
                    });
                    ui.separator();
                    if self.backups.is_empty() {
                        ui.weak("No backups yet.");
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("backups").striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                            for b in &self.backups {
                                let name = b.path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
                                ui.label(RichText::new(name).monospace());
                                let when = b.modified.map_or(String::from("?"), |t| {
                                    chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M:%S").to_string()
                                });
                                ui.label(when);
                                ui.label(format!("{:.1} KB", b.size as f64 / 1024.0));
                                if ui.button("⟲ Restore").on_hover_text("Replace the current save with this backup (the current file is backed up first)").clicked() {
                                    restore = Some(b.path.clone());
                                }
                                if ui.button("📂 Open").on_hover_text("Open the backup itself for viewing/editing").clicked() {
                                    open_backup = Some(b.path.clone());
                                }
                                ui.end_row();
                            }
                        });
                    });
                });
            self.backups_open = open;
            if let Some(p) = restore {
                self.request(Pending::RestoreBackup(p));
            }
            if let Some(p) = open_backup {
                self.request(Pending::Open(p));
            }
        }

        if self.about_open {
            egui::Window::new("About")
                .open(&mut self.about_open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading("Casualties Unknown Save Editor");
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.add_space(6.0);
                    ui.label(
                        "Edits save.sv (gzip-compressed JSON). Unknown fields are preserved as-is.",
                    );
                    ui.label(
                        "Item, limb and setting names are read from the game's own language files.",
                    );
                    ui.label("Built with egui.");
                });
        }
    }

    fn central(&mut self, ui: &mut Ui) {
        let names = Names {
            locale: self.locale.as_ref(),
        };
        let Some(file) = self.save.as_mut() else {
            return;
        };
        let data = &mut file.data;
        match self.tab {
            Tab::Overview => {
                let r = ui::overview::show(ui, data, names);
                if let Some(limb) = r.open_limb {
                    self.limbs.selected = limb;
                    self.tab = Tab::Limbs;
                }
            }
            Tab::Character => {
                ui::character::show(ui, data, names);
            }
            Tab::Health => {
                ui::health::show(ui, data);
            }
            Tab::Limbs => {
                ui::limbs::show(ui, data, names, &mut self.limbs);
            }
            Tab::Inventory => {
                ui::inventory::show(ui, data, names, &mut self.inventory);
            }
            Tab::Run => {
                ui::run::show(ui, data, names);
            }
            Tab::Raw => {
                ui::raw::show(ui, data, &mut self.raw);
            }
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
