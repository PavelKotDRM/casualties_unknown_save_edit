//! Application diagnostics operations.

use super::*;

impl EditorApp {
    // ----- problem reports ---------------------------------------------

    /// Possible problems with the installation, offered for reporting.
    pub(in crate::app) fn problems(&self) -> Vec<Problem> {
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
                "Names, item stats, and recipes are not available; set the folder in Game data → Set game folder…".into(),
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
            add("Game data could not be read from the game", error.clone());
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
    pub(in crate::app) fn diagnostic_report(&self, options: ui::report::ReportOptions) -> String {
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
            Some(error) => format!("- Game data: failed: {error}"),
            None => format!(
                "- Game data: {} items, {} recipes",
                self.item_stats.len(),
                self.crafting.len()
            ),
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

    pub(in crate::app) fn report_action(&mut self, action: ui::report::ReportAction) {
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
}
