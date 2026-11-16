//! About window: the editor's version and information about the installed
//! game (version, compatibility, Steam build and recorded progress).

use egui::{RichText, Ui};

use super::{Names, card, colors};
use crate::achievements::Achievements;
use crate::game_info::{Compatibility, GameInfo, SUPPORTED_GAME_VERSION};
use crate::player_prefs::GameProgress;
use crate::schema;

/// Shows the About window when `open` is set and returns its area.
pub fn show(
    ctx: &egui::Context,
    open: &mut bool,
    names: Names,
    game: Option<&GameInfo>,
    progress: &GameProgress,
    achievements: Option<&Achievements>,
) -> Option<egui::Rect> {
    if !*open {
        return None;
    }
    super::bounded_window(
        ctx,
        format!("ℹ {}", names.text("About")),
        egui::vec2(560.0, 620.0),
        380.0,
    )
    .open(open)
    .show(ctx, |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                editor_card(ui, names);
                game_card(ui, names, game);
                progress_card(ui, names, progress, achievements);
            });
    })
    .map(|shown| shown.response.rect)
}

/// Short game version label for status lines, e.g. `Game v1.0.2`, with a
/// warning colour when the editor was made for another version.
pub fn game_version_label(ui: &mut Ui, names: Names, game: Option<&GameInfo>) {
    let Some(game) = game else {
        return;
    };
    let text = match &game.version {
        Some(version) => names.text("Game v{version}").replace("{version}", version),
        None => names.text("Game version unknown").into_owned(),
    };
    match game.compatibility() {
        Compatibility::Supported => {
            ui.weak(text);
        }
        Compatibility::Different(_) | Compatibility::Unknown => {
            ui.colored_label(colors::WARN, format!("⚠ {text}"))
                .on_hover_text(compatibility_text(names, game));
        }
    }
}

fn compatibility_text(names: Names, game: &GameInfo) -> String {
    match game.compatibility() {
        Compatibility::Supported => names
            .text("The editor's built-in data matches this game version.")
            .into_owned(),
        Compatibility::Different(version) => names
            .text("The editor was made for game version {supported}; version {installed} may add characters, endings or settings it does not know yet. Names, descriptions and item stats are still read from the installed game.")
            .replace("{supported}", SUPPORTED_GAME_VERSION)
            .replace("{installed}", &version),
        Compatibility::Unknown => names
            .text("Could not determine the game version.")
            .into_owned(),
    }
}

fn fact(ui: &mut Ui, key: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        ui.add_sized(
            [170.0, ui.spacing().interact_size.y],
            egui::Label::new(RichText::new(key).weak()).wrap(),
        );
        ui.vertical(add);
    });
}

fn editor_card(ui: &mut Ui, names: Names) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.heading(names.text("Casualties Unknown Save Editor").to_string());
        fact(ui, &names.text("Version"), |ui| {
            ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
        });
        fact(ui, &names.text("Made for game version"), |ui| {
            ui.label(SUPPORTED_GAME_VERSION);
        });
        fact(ui, &names.text("Build"), |ui| {
            let profile = if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            };
            ui.label(format!(
                "{profile} · {} {}",
                std::env::consts::OS,
                std::env::consts::ARCH
            ));
        });
        fact(ui, &names.text("Source code"), |ui| {
            ui.hyperlink(env!("CARGO_PKG_REPOSITORY"));
        });
        ui.add_space(4.0);
        ui.label(
            names
                .text("Edits save.sv (gzip-compressed JSON). Unknown fields are preserved as-is.")
                .to_string(),
        );
        ui.label(
            names
                .text("Item, limb and setting names are read from the game's own language files.")
                .to_string(),
        );
        ui.label(names.text("Built with egui.").to_string());
    });
    ui.add_space(4.0);
}

fn game_card(ui: &mut Ui, names: Names, game: Option<&GameInfo>) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.heading(names.text("Game").to_string());
        let Some(game) = game else {
            ui.colored_label(
                colors::WARN,
                names
                    .text("Game folder not found — set it in the Game data menu.")
                    .to_string(),
            );
            return;
        };
        fact(ui, &names.text("Folder"), |ui| {
            ui.label(RichText::new(game.dir.display().to_string()).monospace());
        });
        fact(ui, &names.text("Version"), |ui| {
            ui.label(game.version.as_deref().unwrap_or("?"));
        });
        let color = match game.compatibility() {
            Compatibility::Supported => colors::GOOD,
            _ => colors::WARN,
        };
        ui.colored_label(color, compatibility_text(names, game));
        if let Some(unity) = &game.unity_version {
            fact(ui, &names.text("Engine"), |ui| {
                ui.label(format!("Unity {unity}"));
            });
        }
        match &game.steam {
            Some(steam) => {
                fact(ui, &names.text("Steam app"), |ui| {
                    ui.hyperlink_to(
                        &steam.app_id,
                        format!("https://store.steampowered.com/app/{}", steam.app_id),
                    );
                });
                fact(ui, &names.text("Steam build"), |ui| {
                    ui.label(&steam.build_id);
                });
                if let Some(updated) = steam
                    .last_updated
                    .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
                {
                    fact(ui, &names.text("Last updated"), |ui| {
                        ui.label(
                            updated
                                .with_timezone(&chrono::Local)
                                .format("%Y-%m-%d %H:%M")
                                .to_string(),
                        );
                    });
                }
                if let Some(size) = steam.size_on_disk {
                    fact(ui, &names.text("Size on disk"), |ui| {
                        ui.label(format!("{:.0} MB", size as f64 / (1024.0 * 1024.0)));
                    });
                }
            }
            None => {
                ui.weak(names.text("Not a Steam installation.").to_string());
            }
        }
    });
    ui.add_space(4.0);
}

fn progress_card(
    ui: &mut Ui,
    names: Names,
    progress: &GameProgress,
    achievements: Option<&Achievements>,
) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.heading(names.text("Progress").to_string());
        let of = |found: usize, total: usize| {
            names
                .text("{found} of {total}")
                .replace("{found}", &found.to_string())
                .replace("{total}", &total.to_string())
        };
        if let Some(achievements) = achievements {
            fact(ui, &names.text("Steam achievements"), |ui| {
                ui.label(of(achievements.unlocked(), achievements.list.len()));
            });
        }
        if let Some(error) = &progress.error {
            ui.colored_label(
                colors::WARN,
                format!("{} {error}", names.text("Could not read the unlocks:")),
            );
            return;
        }
        let characters = (0..schema::CHARACTERS.len())
            .filter(|&i| progress.is_unlocked(i))
            .count();
        fact(ui, &names.text("Characters unlocked"), |ui| {
            ui.label(of(characters, schema::CHARACTERS.len()));
        });
        if let Some(deepest) = progress.deepest_layer() {
            fact(ui, &names.text("Deepest layer reached"), |ui| {
                ui.label(names.layer(deepest));
            });
        }
        if let Some(endings) = progress.discovered_endings() {
            fact(ui, &names.text("Endings discovered"), |ui| {
                ui.label(of(endings, schema::ENDINGS.len()));
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_info::SteamBuild;
    use crate::ui::interface_language::InterfaceLanguage;

    #[test]
    fn long_version_warning_wraps_inside_the_window() {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0));
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let game = GameInfo {
            dir: "D:\\SteamLibrary\\steamapps\\common\\Casualties Unknown".into(),
            version: Some("9.9.9".into()),
            unity_version: Some("2022.3.62f3".into()),
            steam: Some(SteamBuild {
                app_id: "4576490".into(),
                build_id: "26666273".into(),
                last_updated: Some(1_794_661_300),
                size_on_disk: Some(645_457_789),
            }),
        };
        let progress = GameProgress::from_wins(vec![true]).with_endings(vec![true]);
        let mut open = true;
        let mut rect = None;
        for _ in 0..4 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                rect = show(ui.ctx(), &mut open, names, Some(&game), &progress, None);
            });
            output.textures_delta.clear();
        }
        let rect = rect.expect("about window shown");
        assert!(rect.width() <= 600.0, "about window grew to {rect:?}");
    }
}
