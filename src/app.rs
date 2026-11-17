//! The egui application: menus, tabs, undo/redo, backups and file handling.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use egui::{Key, KeyboardShortcut, Modifiers, RichText, Ui};

mod backups;
mod config;
mod files;
mod history;
#[path = "app/diagnostics.rs"]
mod report_actions;
mod view;
use config::Config;
use serde_json::Value;

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
    history: history::UndoStack<Value>,
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
        let (config, spoiler_error) = Config::load();
        let mut app = Self {
            save: None,
            clean: Value::Null,
            history: history::UndoStack::new(Value::Null),
            tab: Tab::Overview,
            status: None,
            last_backup: None,
            config,
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
            progress: GameProgress::unknown("Game data has not been loaded".into()),
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
        if let Some(error) = spoiler_error {
            app.error("Could not read spoiler settings", error);
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
        self.refresh_progress();
        self.refresh_achievements();
        if let Some(dir) = self.config.game_dir.clone() {
            let stamp = GameInfo::stamp(&dir);
            if stamp != self.game_stamp {
                let first = self.game_stamp.is_empty();
                self.game_stamp = stamp;
                let new = GameInfo::read(&dir);
                if !first {
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
                } else if let Some(dir) =
                    locale::detect_game_dir(save::default_save_dir().as_deref())
                    && let Err(error) = self.set_game_dir(Some(dir))
                {
                    self.error("Could not sync item stats", error);
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

    fn game_ready(&self) -> bool {
        !self.item_stats.is_empty() && self.item_sync_error.is_none()
    }

    fn refresh_progress(&mut self) {
        let mut progress = if self.game_ready() {
            GameProgress::load()
        } else {
            GameProgress::unknown(
                "Game data unavailable. Select the game folder in Game data.".into(),
            )
        };
        progress.write_error = self.progress.write_error.clone();
        if self.progress != progress {
            self.wiki.invalidate();
            self.progress = progress;
        }
    }

    fn selected_interface_pack(&self) -> Option<Arc<InterfacePack>> {
        self.interface_packs
            .iter()
            .find(|pack| pack.code == self.config.interface_language)
            .cloned()
    }

    fn reload_interface_packs(&mut self) {
        self.wiki.invalidate();
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
        if dir.is_none() || data.is_err() {
            game_data::runtime::clear();
        }
        let data = data.map(Option::unwrap_or_default);
        let error = data.as_ref().err().cloned();
        let data = data.unwrap_or_default();
        if error.is_some() || dir.is_none() {
            game_data::runtime::clear();
        }
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
        self.wiki.invalidate();
        self.refresh_progress();
        error.map_or(Ok(()), Err)
    }

    fn set_language(&mut self, lang: Option<LanguageFile>) {
        self.wiki.invalidate();
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
        self.wiki.invalidate();
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
        let previous_spoilers = self.config.spoilers.clone();
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
        if self.config.spoilers != previous_spoilers
            && let Err(error) = self.config.store()
        {
            self.error("Could not save editor settings", error);
        }
        if let Some(error) = ctx
            .data_mut(|data| data.remove_temp::<String>(egui::Id::unique("model_operation_error")))
        {
            self.error("Edit failed", error);
        }

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
