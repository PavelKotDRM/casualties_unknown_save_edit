//! Achievement tracker: the game's Steam achievements and which of them the
//! Steam account on this computer has unlocked. Hidden achievements keep
//! their name and description secret until they are unlocked.

use egui::{RichText, Ui};

use super::{Names, card, colors};
use crate::achievements::{Achievement, Achievements};

/// Which achievements to list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    /// Every achievement.
    #[default]
    All,
    /// Unlocked achievements only.
    Unlocked,
    /// Locked achievements only.
    Locked,
}

/// State of the achievements window.
#[derive(Default)]
pub struct AchievementsState {
    /// Whether the window is shown.
    pub open: bool,
    filter: Filter,
    search: String,
    rect: Option<egui::Rect>,
}

/// Steam language of the names: Simplified Chinese for Chinese game or
/// interface languages, English otherwise (the only two the game has).
pub fn steam_language(code: Option<&str>) -> &'static str {
    match code {
        Some(code) if code.to_ascii_lowercase().starts_with("zh") => "schinese",
        _ => "english",
    }
}

/// Text shown for an achievement: hidden ones stay secret until unlocked.
fn visible_text(achievement: &Achievement, lang: &str, names: Names) -> (String, String) {
    if achievement.hidden && !achievement.unlocked {
        (
            names.text("Hidden achievement").into_owned(),
            names.text("Unlock it to see what it is.").into_owned(),
        )
    } else {
        (
            achievement.name(lang).to_owned(),
            achievement.description(lang).to_owned(),
        )
    }
}

/// Shows the window. `data` is the result of reading Steam's cache.
pub fn show(
    ctx: &egui::Context,
    state: &mut AchievementsState,
    names: Names,
    lang: &str,
    data: Option<&Result<Achievements, String>>,
) {
    if !state.open {
        return;
    }
    let mut open = true;
    state.rect = super::bounded_window(
        ctx,
        format!("🏆 {}", names.text("Achievements")),
        egui::vec2(560.0, 640.0),
        360.0,
    )
    .open(&mut open)
    .show(ctx, |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        match data {
            None => {
                ui.weak(names.text("Reading Steam's achievement data…").to_string());
            }
            Some(Err(error)) => {
                ui.colored_label(
                    colors::WARN,
                    format!(
                        "⚠ {} {error}",
                        names.text("Steam's achievement data was not found:")
                    ),
                );
                ui.weak(
                    names
                        .text("Achievements are read from Steam's local cache (appcache/stats); start the game through Steam once to create it.")
                        .to_string(),
                );
            }
            Some(Ok(achievements)) => contents(ui, state, names, lang, achievements),
        }
    })
    .map(|shown| shown.response.rect);
    state.open = open;
}

fn contents(
    ui: &mut Ui,
    state: &mut AchievementsState,
    names: Names,
    lang: &str,
    achievements: &Achievements,
) {
    let total = achievements.list.len();
    let unlocked = achievements.unlocked();
    ui.label(
        RichText::new(
            names
                .text("{found} of {total} unlocked")
                .replace("{found}", &unlocked.to_string())
                .replace("{total}", &total.to_string()),
        )
        .strong(),
    );
    ui.add(
        egui::ProgressBar::new(if total == 0 {
            0.0
        } else {
            unlocked as f32 / total as f32
        })
        .show_percentage(),
    );
    if !achievements.progress_found {
        ui.colored_label(
            colors::WARN,
            names
                .text("No unlocked achievements were found for this computer's Steam account; all are shown as locked.")
                .to_string(),
        );
    }
    ui.weak(
        names
            .text("Read from Steam (read-only); refreshed automatically when Steam updates it.")
            .to_string(),
    );
    ui.horizontal_wrapped(|ui| {
        for (filter, label) in [
            (Filter::All, "All"),
            (Filter::Unlocked, "Unlocked"),
            (Filter::Locked, "Locked"),
        ] {
            ui.selectable_value(&mut state.filter, filter, names.text(label).to_string());
        }
        ui.add(
            egui::TextEdit::singleline(&mut state.search)
                .hint_text(format!("🔍 {}", names.text("Search")))
                .desired_width(160.0),
        );
    });
    ui.separator();
    let search = state.search.to_lowercase();
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let mut shown = 0;
            for achievement in &achievements.list {
                let wanted = match state.filter {
                    Filter::All => true,
                    Filter::Unlocked => achievement.unlocked,
                    Filter::Locked => !achievement.unlocked,
                };
                let (name, description) = visible_text(achievement, lang, names);
                if !wanted
                    || (!search.is_empty()
                        && !name.to_lowercase().contains(&search)
                        && !description.to_lowercase().contains(&search))
                {
                    continue;
                }
                shown += 1;
                row(ui, names, achievement, &name, &description);
            }
            if shown == 0 {
                ui.weak(names.text("Nothing matches.").to_string());
            }
        });
}

fn row(ui: &mut Ui, names: Names, achievement: &Achievement, name: &str, description: &str) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            let (icon, color) = if achievement.unlocked {
                ("🏆", colors::GOOD)
            } else {
                ("🔒", ui.visuals().weak_text_color())
            };
            ui.label(RichText::new(icon).size(22.0).color(color));
            ui.vertical(|ui| {
                let title = RichText::new(name).strong();
                if achievement.unlocked {
                    ui.label(title);
                } else {
                    ui.label(title.weak());
                }
                ui.add(egui::Label::new(RichText::new(description).weak()).wrap());
                if let Some(at) = achievement
                    .unlocked_at
                    .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
                {
                    ui.label(
                        RichText::new(
                            names.text("Unlocked {date}").replace(
                                "{date}",
                                &at.with_timezone(&chrono::Local)
                                    .format("%Y-%m-%d %H:%M")
                                    .to_string(),
                            ),
                        )
                        .small()
                        .color(colors::GOOD),
                    );
                }
            });
        });
    });
    ui.add_space(2.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::interface_language::InterfaceLanguage;

    fn achievement(api: &str, hidden: bool, unlocked: bool) -> Achievement {
        Achievement {
            api_name: api.into(),
            stat: "1".into(),
            bit: 0,
            names: [("english".to_owned(), format!("{api} name"))].into(),
            descriptions: [("english".to_owned(), format!("{api} secret"))].into(),
            hidden,
            unlocked,
            unlocked_at: unlocked.then_some(1_700_000_000),
        }
    }

    #[test]
    fn hidden_achievements_stay_secret_and_the_window_fits() {
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let locked = achievement("BIOPOD", true, false);
        let (name, description) = visible_text(&locked, "english", names);
        assert!(!name.contains("BIOPOD") && !description.contains("secret"));
        let unlocked = achievement("GRADUATE", true, true);
        assert_eq!(visible_text(&unlocked, "english", names).0, "GRADUATE name");
        assert_eq!(steam_language(Some("zh-CN")), "schinese");
        assert_eq!(steam_language(Some("EN")), "english");

        let data = Ok(Achievements {
            list: (0..40)
                .map(|i| {
                    achievement(
                        &format!("A{i} with a rather long name"),
                        i % 3 == 0,
                        i % 2 == 0,
                    )
                })
                .collect(),
            progress_found: true,
            files: Vec::new(),
        });
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 500.0));
        let mut state = AchievementsState {
            open: true,
            ..Default::default()
        };
        for _ in 0..4 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                show(ui.ctx(), &mut state, names, "english", Some(&data));
            });
            output.textures_delta.clear();
        }
        let rect = state.rect.expect("achievements window shown");
        assert!(
            screen.contains_rect(rect),
            "window {rect:?} leaves the screen"
        );
    }
}
