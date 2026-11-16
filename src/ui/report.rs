//! "Report a problem" window: shows the anonymised debugging information,
//! lets the user review and edit it, and hands it to GitHub, the clipboard or
//! a file. Nothing is sent by the editor itself.

use egui::{RichText, Ui};

use super::{Names, colors};
use crate::diagnostics::Problem;

/// What to include in the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportOptions {
    /// Errors shown during this session.
    pub errors: bool,
    /// The latest crash report.
    pub crash: bool,
    /// Game progress and a summary of the open save (no save contents).
    pub progress: bool,
}

impl Default for ReportOptions {
    fn default() -> Self {
        Self {
            errors: true,
            crash: true,
            progress: false,
        }
    }
}

/// What the user asked for; carried out by the application.
#[derive(Debug, Clone, PartialEq)]
pub enum ReportAction {
    /// Open a pre-filled GitHub issue in the browser.
    OpenIssue {
        /// Issue title.
        title: String,
        /// The user's description.
        description: String,
        /// The (reviewed) debugging information.
        diagnostics: String,
    },
    /// Copy the whole report to the clipboard.
    Copy(String),
    /// Save the whole report to a file.
    Save(String),
    /// Delete the stored crash reports.
    DeleteCrashReports,
}

/// State of the report window.
#[derive(Default)]
pub struct ReportState {
    /// Whether the window is shown.
    pub open: bool,
    title: String,
    description: String,
    options: ReportOptions,
    /// Debugging information shown to and editable by the user.
    text: String,
    /// Options the text was generated with (`None`: generate again).
    generated: Option<ReportOptions>,
    rect: Option<egui::Rect>,
}

impl ReportState {
    /// Opens the window, suggesting `title` for the issue.
    pub fn open_with(&mut self, title: impl Into<String>) {
        self.open = true;
        self.title = title.into();
        self.generated = None;
    }

    /// Options to generate the debugging information with, when it has to be
    /// (re)generated.
    pub fn wanted(&self) -> Option<ReportOptions> {
        (self.open && self.generated != Some(self.options)).then_some(self.options)
    }

    /// Generates the debugging information again (discarding edits).
    pub fn invalidate(&mut self) {
        self.generated = None;
    }

    /// Sets freshly generated debugging information.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.generated = Some(self.options);
    }

    /// The report as one Markdown document.
    fn full_text(&self) -> String {
        let mut text = String::new();
        if !self.title.trim().is_empty() {
            text.push_str(&format!("## {}\n\n", self.title.trim()));
        }
        if !self.description.trim().is_empty() {
            text.push_str(&format!("{}\n\n", self.description.trim()));
        }
        text.push_str(&self.text);
        text
    }
}

/// Shows the window. `problems` are possible problems noticed in the
/// installation; `crash` tells whether a crash report is stored.
pub fn show(
    ctx: &egui::Context,
    state: &mut ReportState,
    names: Names,
    problems: &[Problem],
    crash: bool,
) -> Option<ReportAction> {
    if !state.open {
        return None;
    }
    let mut action = None;
    let mut open = true;
    state.rect = super::bounded_window(
        ctx,
        format!("🐞 {}", names.text("Report a problem")),
        egui::vec2(680.0, 720.0),
        420.0,
    )
    .open(&mut open)
    .show(ctx, |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                contents(ui, state, names, problems, crash, &mut action)
            });
    })
    .map(|shown| shown.response.rect);
    state.open = open;
    action
}

fn contents(
    ui: &mut Ui,
    state: &mut ReportState,
    names: Names,
    problems: &[Problem],
    crash: bool,
    action: &mut Option<ReportAction>,
) {
    ui.label(
        RichText::new(
            names
                .text("🔒 Nothing is sent automatically. Review the text below: it is exactly what will be shared. Your user and computer names, home folder, Steam account and e-mail addresses are replaced with placeholders, and save contents are never included.")
                .to_string(),
        )
        .color(colors::GOOD),
    );
    ui.add_space(4.0);
    if !problems.is_empty() {
        ui.label(RichText::new(names.text("Possible problems found").to_string()).strong());
        for problem in problems {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(colors::WARN, format!("⚠ {}", names.text(&problem.title)));
                if ui
                    .small_button(names.text("Use as title").to_string())
                    .clicked()
                {
                    state.title = problem.title.clone();
                }
            });
            ui.weak(&problem.detail);
        }
        ui.separator();
    }
    ui.label(names.text("Title").to_string());
    ui.add(
        egui::TextEdit::singleline(&mut state.title)
            .hint_text(names.text("Short summary of the problem").to_string())
            .desired_width(f32::INFINITY),
    );
    ui.label(names.text("What happened?").to_string());
    ui.add(
        egui::TextEdit::multiline(&mut state.description)
            .hint_text(
                names
                    .text("What did you do, what did you expect, and what happened instead?")
                    .to_string(),
            )
            .desired_rows(4)
            .desired_width(f32::INFINITY),
    );
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(names.text("Include:").to_string());
        ui.checkbox(&mut state.options.errors, names.text("Recent errors").to_string());
        ui.add_enabled_ui(crash, |ui| {
            ui.checkbox(&mut state.options.crash, names.text("Last crash").to_string());
        });
        ui.checkbox(
            &mut state.options.progress,
            names.text("Game progress and save summary").to_string(),
        )
        .on_hover_text(
            names
                .text("Unlocked characters, endings and achievements count, and the character, layer and run time of the open save")
                .to_string(),
        );
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(names.text("Debugging information").to_string()).strong());
        if ui
            .small_button(format!("⟳ {}", names.text("Regenerate")))
            .on_hover_text(names.text("Discard your edits to this text").to_string())
            .clicked()
        {
            state.generated = None;
        }
    });
    ui.add(
        egui::TextEdit::multiline(&mut state.text)
            .code_editor()
            .desired_rows(14)
            .desired_width(f32::INFINITY),
    );
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        let ready = !state.title.trim().is_empty();
        if ui
            .add_enabled(
                ready,
                egui::Button::new(
                    RichText::new(format!("🌐 {}", names.text("Create GitHub issue…"))).strong(),
                ),
            )
            .on_hover_text(
                names
                    .text("Opens a pre-filled issue in your browser; you can still edit it there before submitting. Needs a GitHub account.")
                    .to_string(),
            )
            .on_disabled_hover_text(names.text("Enter a title first").to_string())
            .clicked()
        {
            *action = Some(ReportAction::OpenIssue {
                title: state.title.trim().to_owned(),
                description: state.description.trim().to_owned(),
                diagnostics: state.text.clone(),
            });
        }
        if ui.button(format!("📋 {}", names.text("Copy"))).clicked() {
            *action = Some(ReportAction::Copy(state.full_text()));
        }
        if ui.button(format!("💾 {}", names.text("Save to file…"))).clicked() {
            *action = Some(ReportAction::Save(state.full_text()));
        }
        if crash
            && ui
                .button(format!("🗑 {}", names.text("Delete crash reports")))
                .clicked()
        {
            *action = Some(ReportAction::DeleteCrashReports);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::interface_language::InterfaceLanguage;

    #[test]
    fn report_window_regenerates_on_option_change_and_fits() {
        let mut state = ReportState::default();
        assert_eq!(state.wanted(), None, "closed window needs no report");
        state.open_with("Crash");
        let options = state.wanted().expect("text generated on open");
        state.set_text(format!("### Environment\n\n- {}\n", "x".repeat(400)));
        assert_eq!(state.wanted(), None);
        state.options.progress = !options.progress;
        assert!(
            state.wanted().is_some(),
            "regenerated after an option change"
        );
        state.set_text("text".into());
        assert!(state.full_text().starts_with("## Crash\n\n"));

        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let problems = [Problem {
            title: "Game folder not found".into(),
            detail: "y".repeat(300),
        }];
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 500.0));
        for _ in 0..4 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                show(ui.ctx(), &mut state, names, &problems, true);
            });
            output.textures_delta.clear();
        }
        let rect = state.rect.expect("report window shown");
        assert!(
            screen.contains_rect(rect),
            "window {rect:?} leaves the screen"
        );
    }
}
