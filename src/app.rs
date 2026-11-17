//! The egui application: menus, tabs, undo/redo, backups and file handling.

// FIXME: 1250+ lines; `impl EditorApp` is a god object covering config persistence (`Config`),
// font setup, undo/redo history (`commit`/`undo`/`redo`/`reset_history`), file I/O
// (`load`/`save`/`save_as`/`export_json`/`import_json`/`create_new_save`), the new-save wizard,
// backups (`manual_backup`/`restore_backup`/`backup_action`), achievements/interface-pack refresh,
// problem detection and a ~160-line `diagnostic_report`. Extract: `app/config.rs` (Config +
// load/store), `app/history.rs` (an `UndoStack<Value>` with MAX_UNDO), `app/files.rs` (open/save/
// import/export/new-save actions returning `Result` instead of calling `self.error` inline),
// `app/backups.rs`, and `app/diagnostics.rs` (`problems`, `diagnostic_report`, `report_action`).
// Keep this file for `EditorApp` fields, `new`, `shortcuts` and the `eframe::App` impl.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use egui::{Key, KeyboardShortcut, Modifiers, RichText, Ui};

mod view;
use serde_json::{Value, json};

use crate::achievements::{self, Achievements};
use crate::diagnostics::{self, Problem, Redactor};
use crate::game_data::{self, ItemStats};
use crate::game_info::GameInfo;
use crate::interface_packs::{self, InterfacePack};
use crate::locale::{self, LanguageFile, Locale};
use crate::platform;
use crate::player_prefs::GameProgress;
use crate::save::{self, SaveError, SaveFile, SaveFormat};
use crate::ui::{self, InterfaceLanguage, Names, colors};

const MAX_UNDO: usize = 200;
/// How often the save, unlocks, achievements and game data are re-read.
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const CTRL_S: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const CTRL_O: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const CTRL_Z: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const CTRL_Y: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const CTRL_SHIFT_Z: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);

/// First lines of a crash report's panic message.
fn crash_summary(text: &str) -> String {
    text.lines()
        .skip(2)
        .take_while(|l| !l.starts_with("Backtrace"))
        .filter(|l| !l.trim().is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join(" ")
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Persistent user settings (`cu_save_editor/config.json` in the OS configuration folder).
struct Config {
    game_dir: Option<PathBuf>,
    language: Option<String>,
    interface_language: String,
    /// Re-read the save, unlocks, achievements and game data periodically.
    auto_refresh: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            game_dir: None,
            language: None,
            interface_language: "en".to_owned(),
            auto_refresh: true,
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
            auto_refresh: v
                .get("autoRefresh")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }

    fn store(&self) -> Result<(), String> {
        let path = Self::path().ok_or("Cannot determine the editor settings folder")?;
        let v = json!({
            "gameDir": self.game_dir.as_ref().map(|p| p.to_string_lossy()),
            "language": self.language,
            "interfaceLanguage": self.interface_language,
            "autoRefresh": self.auto_refresh,
        });
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&path, save::to_pretty_json(&v))
            .map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Adds a system font ([`platform::font_candidates`]) as a fallback for
/// glyphs missing from egui's built-in fonts (symbols, CJK item names).
/// Returns `false` if no candidate file could be read.
fn add_fallback_font(ctx: &egui::Context, name: &str, kind: platform::FallbackFont) -> bool {
    let Some(bytes) = platform::font_candidates(kind)
        .into_iter()
        .find_map(|path| std::fs::read(path).ok())
    else {
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
    item_effects: game_data::GameEffects,
    game_info: Option<GameInfo>,
    limbs: ui::limbs::LimbsState,
    inventory: ui::inventory::InventoryState,
    raw: ui::raw::TreeState,
    backups: ui::backups::BackupBrowser,
    about_open: bool,
    pending: Option<Pending>,
    new_save_wizard: Option<NewSaveWizard>,
    progress: GameProgress,
    achievements: Option<Result<Achievements, String>>,
    achievements_stamp: Option<Vec<(PathBuf, Option<SystemTime>)>>,
    achievements_ui: ui::achievements::AchievementsState,
    /// When external data was last checked ([`Self::refresh_external`]).
    last_refresh: Instant,
    /// Fingerprint of the game files ([`GameInfo::stamp`]).
    game_stamp: Vec<Option<SystemTime>>,
    /// Hash of the open save's file as last seen on disk (`None` inside:
    /// the file was missing), so each change is reported once.
    disk_seen: Option<Option<u64>>,
    wiki: ui::wiki::WikiState,
    allow_close: bool,
    title: String,
    ctx: egui::Context,
    cjk_font: bool,
    /// Errors shown in this session, for bug reports.
    error_log: diagnostics::ErrorLog,
    /// Why item stats could not be read from the game, if they could not.
    item_sync_error: Option<String>,
    /// Latest crash report: path and text.
    crash: Option<(PathBuf, String)>,
    report: ui::report::ReportState,
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
            item_effects: Default::default(),
            game_info: None,
            limbs: Default::default(),
            inventory: Default::default(),
            raw: Default::default(),
            backups: Default::default(),
            about_open: false,
            pending: None,
            new_save_wizard: None,
            progress: GameProgress::load(),
            achievements: None,
            achievements_stamp: None,
            achievements_ui: Default::default(),
            last_refresh: Instant::now(),
            game_stamp: Vec::new(),
            disk_seen: None,
            wiki: Default::default(),
            allow_close: false,
            title: String::new(),
            ctx: cc.egui_ctx.clone(),
            cjk_font: false,
            error_log: Default::default(),
            item_sync_error: None,
            crash: diagnostics::crash_dir().and_then(|d| diagnostics::latest_crash_report(&d)),
            report: Default::default(),
        };
        add_fallback_font(&cc.egui_ctx, "symbols", platform::FallbackFont::Symbols);
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
        app.refresh_achievements();
        if app.crash.is_some() && app.status.as_ref().is_none_or(|s| s.1 != Level::Error) {
            app.set_status(
                "The editor closed unexpectedly last time — Help → Report a problem includes the crash details",
                Level::Info,
            );
        }
        app
    }

    // ----- external data ----------------------------------------------

    /// Re-reads the achievements if Steam's cache files changed.
    fn refresh_achievements(&mut self) {
        let stamp = achievements::fingerprint(platform::STEAM_APP_ID);
        if self.achievements_stamp.as_ref() != Some(&stamp) {
            self.achievements = Some(Achievements::load(platform::STEAM_APP_ID));
            self.achievements_stamp = Some(stamp);
        }
    }

    /// Steam language for achievement names.
    fn achievement_language(&self) -> &'static str {
        ui::achievements::steam_language(
            self.config
                .language
                .as_deref()
                .or(Some(self.config.interface_language.as_str())),
        )
    }

    /// Re-reads everything that can change outside the editor: the game's
    /// progress, Steam achievements, the game installation (updates), the
    /// backup list and the open save on disk.
    fn refresh_external(&mut self) {
        self.last_refresh = Instant::now();
        let write_error = self.progress.write_error.take();
        self.progress = GameProgress::load();
        self.progress.write_error = write_error;
        self.refresh_achievements();
        if let Some(dir) = self.config.game_dir.clone() {
            let stamp = GameInfo::stamp(&dir);
            if stamp != self.game_stamp {
                let first = self.game_stamp.is_empty();
                self.game_stamp = stamp;
                let old = self.game_info.clone();
                let new = GameInfo::read(&dir);
                let changed = old.as_ref().map(|g| (&g.version, &g.steam))
                    != Some((&new.version, &new.steam));
                if !first && changed {
                    match self.set_game_dir(Some(dir)) {
                        Ok(()) => self.set_status(
                            format!(
                                "The game was updated (version {}) — game data reloaded",
                                new.version.as_deref().unwrap_or("?")
                            ),
                            Level::Info,
                        ),
                        Err(error) => self.error("Could not sync item stats", error),
                    }
                }
            }
        }
        if self.backups.open {
            let target = self.backups.target().map(Path::to_path_buf);
            self.backups.refresh_if_changed(target);
        }
        self.check_open_save();
    }

    /// Notices when the game (or anything else) changed or removed the open
    /// save: an unmodified document is reloaded, otherwise the user is told.
    fn check_open_save(&mut self) {
        let Some(file) = &self.save else {
            self.disk_seen = None;
            return;
        };
        let path = file.path.clone();
        let bytes = std::fs::read(&path).ok();
        let seen = bytes.as_deref().map(hash_bytes);
        if self.disk_seen == Some(seen) {
            return;
        }
        self.disk_seen = Some(seen);
        match bytes {
            None => self.set_status(
                "The save file is no longer on disk (the game deletes save.sv when it loads the run); saving writes it again",
                Level::Info,
            ),
            Some(bytes) if bytes == file.disk_bytes => {}
            Some(_) if !self.is_dirty() => {
                self.load(&path, false);
                self.set_status(
                    format!("{} changed on disk and was reloaded", path.display()),
                    Level::Info,
                );
            }
            Some(_) => self.set_status(
                "The save changed on disk (did the game save?) — File → Reload from disk loads it; saving keeps your edits and backs the disk version up",
                Level::Error,
            ),
        }
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
            self.cjk_font = add_fallback_font(&self.ctx, "cjk", platform::FallbackFont::Cjk);
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
        let data = dir.as_deref().map(game_data::load_game_data).transpose();
        let data = data.map(Option::unwrap_or_default);
        let error = data.as_ref().err().cloned();
        let data = data.unwrap_or_default();
        self.item_stats = data.items;
        self.item_effects = data.effects;
        self.game_info = dir.as_deref().map(GameInfo::read);
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
        self.item_sync_error = error.clone();
        error.map_or(Ok(()), Err)
    }

    fn set_language(&mut self, lang: Option<LanguageFile>) {
        self.locale = lang.as_ref().and_then(|l| Locale::load(&l.path));
        if let Some(l) = lang {
            if l.code.starts_with("zh") && !self.cjk_font {
                self.cjk_font = add_fallback_font(&self.ctx, "cjk", platform::FallbackFont::Cjk);
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
            self.cjk_font = add_fallback_font(&self.ctx, "cjk", platform::FallbackFont::Cjk);
        }
        if let Err(error) = self.config.store() {
            self.error("Could not save editor settings", error);
        }
    }

    // ----- status -----------------------------------------------------

    fn set_status(&mut self, text: impl Into<String>, level: Level) {
        let text = text.into();
        if level == Level::Error {
            self.error_log.push(text.clone());
        }
        self.status = Some((text, level, Instant::now()));
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
                self.progress = GameProgress::load();
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

    /// Save whose backups the browser shows: the open save (or, for an open
    /// backup, the save it was made from), else the game's default save.
    fn backup_target(&self) -> Option<PathBuf> {
        match self.save.as_ref().map(|s| &s.path) {
            Some(path) if save::is_backup(path) => save::backup_source(path),
            Some(path) => Some(path.clone()),
            None => save::default_save_path(),
        }
    }

    fn open_backups(&mut self) {
        let target = self.backup_target();
        self.backups.show_for(target);
    }

    fn manual_backup(&mut self) {
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
    fn restore_backup(&mut self, backup: &Path) {
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

    fn backup_action(&mut self, action: ui::backups::BackupAction) {
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

    // ----- problem reports ---------------------------------------------

    /// Possible problems with the installation, offered for reporting.
    fn problems(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        let mut add = |title: &str, detail: String| {
            problems.push(Problem {
                title: title.to_owned(),
                detail,
            })
        };
        if let Some((_, text)) = &self.crash {
            add("The editor crashed", crash_summary(text));
        }
        match (&self.config.game_dir, &self.game_info) {
            (None, _) => add(
                "Game folder not found",
                "Names and item stats are not available; set the folder in Game data → Set game folder…".into(),
            ),
            (Some(_), Some(game)) => {
                if let crate::game_info::Compatibility::Different(version) = game.compatibility() {
                    add(
                        "Unsupported game version",
                        format!(
                            "Installed {version}, the editor was made for {}",
                            crate::game_info::SUPPORTED_GAME_VERSION
                        ),
                    );
                }
            }
            _ => {}
        }
        if let Some(error) = &self.item_sync_error {
            add("Item stats could not be read from the game", error.clone());
        }
        if let Some(error) = &self.progress.error {
            add("The game's preferences could not be read", error.clone());
        }
        if let Some(error) = &self.progress.write_error {
            add("The game's preferences could not be changed", error.clone());
        }
        if self.game_info.as_ref().is_some_and(|g| g.steam.is_some())
            && let Some(Err(error)) = &self.achievements
        {
            add("Steam achievement data was not read", error.clone());
        }
        if save::default_save_dir().is_none() {
            add(
                "Save folder not found",
                "The game's save folder could not be determined on this system".into(),
            );
        }
        problems
    }

    /// Debugging information for a bug report, redacted.
    fn diagnostic_report(&self, options: ui::report::ReportOptions) -> String {
        let yes_no = |b: bool| if b { "yes" } else { "no" };
        let mut report = diagnostics::Report::default();
        report.section(
            "Environment",
            vec![
                format!(
                    "- Editor: v{} ({}, {} {})",
                    env!("CARGO_PKG_VERSION"),
                    if cfg!(debug_assertions) {
                        "debug"
                    } else {
                        "release"
                    },
                    std::env::consts::OS,
                    std::env::consts::ARCH
                ),
                format!(
                    "- Interface language: {}; game names: {}",
                    self.config.interface_language,
                    self.config.language.as_deref().unwrap_or("-")
                ),
                format!("- Automatic refresh: {}", yes_no(self.config.auto_refresh)),
            ],
        );
        let mut game = Vec::new();
        match (&self.config.game_dir, &self.game_info) {
            (Some(dir), info) => {
                game.push(format!("- Folder: `{}`", dir.display()));
                if let Some(info) = info {
                    game.push(format!(
                        "- Version: {} (editor made for {})",
                        info.version.as_deref().unwrap_or("unknown"),
                        crate::game_info::SUPPORTED_GAME_VERSION
                    ));
                    if let Some(unity) = &info.unity_version {
                        game.push(format!("- Unity: {unity}"));
                    }
                    if let Some(steam) = &info.steam {
                        game.push(format!("- Steam build: {}", steam.build_id));
                    }
                }
            }
            (None, _) => game.push("- Folder: not found".into()),
        }
        game.push(match &self.item_sync_error {
            Some(error) => format!("- Item stats: failed: {error}"),
            None => format!("- Item stats: {} items", self.item_stats.len()),
        });
        game.push(format!(
            "- Language files: {}",
            self.languages
                .iter()
                .map(|l| l.code.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        report.section("Game", game);

        let prefs = crate::player_prefs::store();
        let mut data = vec![
            format!(
                "- Save folder: {}",
                save::default_save_dir().map_or("not found".into(), |d| format!(
                    "`{}` (exists: {})",
                    d.display(),
                    yes_no(d.is_dir())
                ))
            ),
            format!("- Game preferences: {}", prefs.location()),
        ];
        if let Some(error) = &self.progress.error {
            data.push(format!("- Preferences error: {error}"));
        }
        if let Some(error) = &self.progress.write_error {
            data.push(format!("- Preferences write error: {error}"));
        }
        data.push(match &self.achievements {
            Some(Ok(a)) => format!(
                "- Steam achievements: {} defined, progress found: {}",
                a.list.len(),
                yes_no(a.progress_found)
            ),
            Some(Err(error)) => format!("- Steam achievements: {error}"),
            None => "- Steam achievements: not read".into(),
        });
        report.section("Data sources", data);

        if let Some(file) = &self.save {
            let mut lines = vec![
                format!("- File: `{}`", file.path.display()),
                format!(
                    "- Format: {:?}, {} bytes on disk, backup: {}",
                    file.format,
                    file.disk_bytes.len(),
                    yes_no(save::is_backup(&file.path))
                ),
                format!(
                    "- Unsaved edits: {}; changed on disk since loading: {}",
                    yes_no(self.is_dirty()),
                    yes_no(file.changed_on_disk())
                ),
            ];
            if options.progress {
                let s = save::SaveSummary::of(&file.data);
                lines.push(format!(
                    "- Character {:?}, layer {:?}, run time {:?} s, items {}, skills {:?}, cheated: {}",
                    s.character, s.biome, s.run_time, s.items, s.skills, yes_no(s.cheated)
                ));
            }
            report.section("Open save", lines);
        }
        if options.progress && self.progress.error.is_none() {
            let characters = (0..crate::schema::CHARACTERS.len())
                .filter(|&i| self.progress.is_unlocked(i))
                .count();
            let mut lines = vec![
                format!(
                    "- Characters unlocked: {characters} of {}",
                    crate::schema::CHARACTERS.len()
                ),
                format!("- Deepest layer: {:?}", self.progress.deepest_layer()),
                format!(
                    "- Endings discovered: {:?} of {}",
                    self.progress.discovered_endings(),
                    crate::schema::ENDINGS.len()
                ),
            ];
            if let Some(Ok(a)) = &self.achievements {
                lines.push(format!(
                    "- Achievements unlocked: {} of {}",
                    a.unlocked(),
                    a.list.len()
                ));
            }
            report.section("Game progress", lines);
        }
        report.section(
            "Possible problems",
            self.problems()
                .iter()
                .map(|p| format!("- {}: {}", p.title, p.detail))
                .collect(),
        );
        if options.errors {
            report.section(
                "Recent errors",
                self.error_log
                    .entries()
                    .map(|e| format!("- {} {}", e.at.format("%H:%M:%S"), e.message))
                    .collect(),
            );
        }
        if options.crash
            && let Some((_, text)) = &self.crash
        {
            report.section(
                "Last crash",
                vec!["```".into(), text.trim().to_owned(), "```".into()],
            );
        }
        report.to_markdown(&Redactor::for_this_computer())
    }

    fn report_action(&mut self, action: ui::report::ReportAction) {
        use ui::report::ReportAction;
        let redactor = Redactor::for_this_computer();
        match action {
            ReportAction::OpenIssue {
                title,
                description,
                diagnostics,
            } => {
                let diagnostics = redactor.redact(&diagnostics);
                let (url, included) = diagnostics::issue_url(
                    env!("CARGO_PKG_REPOSITORY"),
                    &redactor.redact(&title),
                    &redactor.redact(&description),
                    &diagnostics,
                );
                if included {
                    self.set_status("Opening the issue form in your browser", Level::Success);
                } else {
                    self.ctx.copy_text(diagnostics);
                    self.set_status(
                        "The debugging information is too long for a link and was copied — paste it into the issue's Diagnostics field",
                        Level::Info,
                    );
                }
                self.ctx.open_url(egui::OpenUrl::new_tab(url));
            }
            ReportAction::Copy(text) => {
                self.ctx.copy_text(redactor.redact(&text));
                self.set_status("Report copied to the clipboard", Level::Success);
            }
            ReportAction::Save(text) => {
                let name = format!(
                    "cu_save_editor_report_{}.md",
                    chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
                );
                let Some(path) = rfd::FileDialog::new()
                    .set_title("Save report")
                    .set_file_name(name)
                    .add_filter("Markdown", &["md"])
                    .save_file()
                else {
                    return;
                };
                match std::fs::write(&path, redactor.redact(&text)) {
                    Ok(()) => self.set_status(
                        format!("Report saved to {}", path.display()),
                        Level::Success,
                    ),
                    Err(e) => self.error("Could not save the report", e),
                }
            }
            ReportAction::DeleteCrashReports => {
                let deleted =
                    diagnostics::crash_dir().map_or(0, |d| diagnostics::delete_crash_reports(&d));
                self.crash = None;
                self.report.invalidate();
                self.set_status(format!("Deleted {deleted} crash report(s)"), Level::Success);
            }
        }
    }

    fn open_folder(path: &Path) {
        let _ = platform::open_folder(path);
    }

    // ----- UI ---------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let text_focused = ctx.memory(|m| m.focused().is_some());
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F5)) {
            self.refresh_external();
            self.set_status("Data refreshed", Level::Success);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&CTRL_S)) && self.save.is_some() {
            self.save();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&CTRL_O)) {
            self.request(Pending::OpenDialog);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F1)) {
            self.wiki.open = !self.wiki.open;
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
        if let Some(options) = self.report.wanted() {
            let text = self.diagnostic_report(options);
            self.report.set_text(text);
        }
        if self.config.auto_refresh {
            if self.last_refresh.elapsed() >= REFRESH_INTERVAL {
                self.refresh_external();
            }
            ctx.request_repaint_after(REFRESH_INTERVAL.saturating_sub(self.last_refresh.elapsed()));
        }
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
