//! Character tab: identity of the experiment and its skills.

use egui::{ProgressBar, RichText, Ui};
use serde_json::{Map, Value};

use super::{LABEL_WIDTH, Names, card, colors, number_editor, section_heading};
use crate::json::{get_f64, get_i64, set_i64};
use crate::player_prefs::GameProgress;
use crate::{model, save, schema};

/// Top-level identity keys shown on this tab, in `schema::IDENTITY_KEYS` order.
const IDENTITY: &[(&str, &str, &str)] = &[
    (
        "cHeight",
        "Height (cm)",
        "Character height. The game rolls it from a range scaled by the character's size.",
    ),
    ("cAge", "Age", "Character age in years."),
    ("cId", "Experiment id", "Experiment number shown in game."),
    ("cVer", "Version", "Character generation version."),
];

fn label(ui: &mut Ui, names: Names, text: &str, help: &str) {
    ui.add_sized(
        [LABEL_WIDTH, ui.spacing().interact_size.y],
        egui::Label::new(names.text(text).to_string()),
    )
    .on_hover_text(names.text(help).to_string());
}

/// Character selector for the `character` field. Characters that are still
/// locked in the game cannot be chosen. Returns the newly selected character.
pub fn character_combo(
    ui: &mut Ui,
    root: &mut Map<String, Value>,
    names: Names,
    unlocks: &GameProgress,
) -> Option<usize> {
    let Some(current) = get_i64(root, "character") else {
        ui.weak("—");
        return None;
    };
    let mut value = current;
    egui::ComboBox::from_id_salt("character")
        .selected_text(names.character(value))
        .width(260.0)
        .show_ui(ui, |ui| {
            for i in 0..schema::CHARACTERS.len() {
                let unlocked = unlocks.is_unlocked(i);
                let label = names.character_if_unlocked(i, unlocks);
                let response =
                    ui.add_enabled(unlocked, egui::Button::selectable(value == i as i64, label));
                if response.clicked() {
                    value = i as i64;
                }
                response.on_disabled_hover_text(names.character_unlock_requirement());
            }
        });
    if value != current {
        set_i64(root, "character", value);
        usize::try_from(value).ok()
    } else {
        None
    }
}

/// Starting skills, dossier and quirks of a character.
fn character_info(ui: &mut Ui, names: Names, character: usize) {
    let Some(def) = schema::CHARACTERS.get(character) else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.add_space(LABEL_WIDTH + ui.spacing().item_spacing.x);
        ui.weak(names.text("Starting skills:").to_string());
        for (skill, (level, mult)) in schema::SKILLS
            .iter()
            .zip(def.base_skills.iter().zip(def.gain_multipliers))
        {
            let mut text = format!("{} {level}", skill.key);
            if mult != 1.0 {
                text.push_str(&format!(" (×{} XP)", super::fmt_num(mult)));
            }
            ui.label(RichText::new(text).monospace())
                .on_hover_text(names.skill(skill));
        }
    });
    let description = names.character_description(character);
    let quirks = names.character_quirks(character);
    if description.is_some() || quirks.is_some() {
        egui::CollapsingHeader::new(names.text("Dossier and quirks").to_string())
            .id_salt(("character_dossier", character))
            .show(ui, |ui| {
                if let Some(text) = description {
                    ui.label(text);
                }
                if let Some(text) = quirks {
                    ui.add_space(4.0);
                    ui.label(RichText::new(text).color(colors::ACCENT));
                }
            });
    }
}

/// Editor of the game's character unlocks (stored outside the save).
pub fn unlocks_card(ui: &mut Ui, names: Names, unlocks: &mut GameProgress) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.strong(names.text("Unlocked characters").to_string());
            if ui
                .small_button(format!("⟳ {}", names.text("Refresh")))
                .on_hover_text(
                    names
                        .text("Read the unlocks from the game again")
                        .to_string(),
                )
                .clicked()
            {
                *unlocks = GameProgress::load();
            }
        });
        ui.weak(
            names
                .text("Stored in the game's settings, not in the save. Character N unlocks after a run is won as character N − 1. Close the game before changing them.")
                .to_string(),
        );
        if let Some(error) = &unlocks.error {
            ui.colored_label(
                colors::WARN,
                format!("{} {error}", names.text("Could not read the unlocks:")),
            );
            return;
        }
        for i in 0..schema::CHARACTERS.len() {
            let mut unlocked = unlocks.is_unlocked(i);
            let response = ui.add_enabled(
                i != 0,
                egui::Checkbox::new(&mut unlocked, names.character_if_unlocked(i, unlocks)),
            );
            if i == 0 {
                response.on_disabled_hover_text(names.text("Always unlocked").to_string());
            } else if response.changed() {
                unlocks.write_error = unlocks.set_unlocked(i, unlocked).err();
            }
        }
        if let Some(error) = &unlocks.write_error {
            ui.colored_label(
                colors::BAD,
                format!("{} {error}", names.text("Could not change the unlock:")),
            );
        }
    });
}

/// Endings the player has reached, read from (and editable in) the game's
/// preferences. Undiscovered details require explicit, staged spoiler consent.
pub fn endings_card(
    ui: &mut Ui,
    names: Names,
    progress: &mut GameProgress,
    spoilers: &mut super::spoilers::SpoilerUi<'_>,
) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.strong(names.text("Endings").to_string());
            if let Some(found) = progress.discovered_endings() {
                ui.weak(
                    names
                        .text("{found} discovered")
                        .replace("{found}", &found.to_string()),
                );
            }
        });
        ui.horizontal_wrapped(|ui| super::spoilers::ending_total(ui, names, spoilers.state));
        if let Some(error) = &progress.error {
            ui.colored_label(
                colors::WARN,
                format!("{} {error}", names.text("Could not read the unlocks:")),
            );
        }
        for i in 0..schema::ENDINGS.len() {
            if !spoilers.state.ending_visible(i, progress) {
                continue;
            }
            let mut discovered = progress.is_ending_discovered(i);
            let response = ui.add_enabled(
                progress.is_known(),
                egui::Checkbox::new(
                    &mut discovered,
                    spoilers.hints.title(i, names, progress, spoilers.state),
                ),
            );
            let response = if discovered {
                response.on_hover_text(names.text(schema::ENDINGS[i].how).to_string())
            } else {
                response.on_hover_text(names.text("Not discovered yet.").to_string())
            };
            if response.changed() {
                progress.write_error = progress.set_ending_discovered(i, discovered).err();
            }
            if !discovered {
                ui.push_id(("ending_hints", i), |ui| {
                    for hint in spoilers.hints.visible(i, spoilers.state) {
                        ui.label(format!("{}: {}", names.text(hint.label), hint.text));
                    }
                    super::spoilers::hint_button(ui, names, i, spoilers.state, &spoilers.hints);
                });
            }
        }
        super::spoilers::next_ending(ui, names, progress, spoilers.state, &spoilers.hints);
        ui.weak(
            names
                .text("The game marks an ending again on start if you have its Steam achievement.")
                .to_string(),
        );
    });
}

/// Shows the character tab. Returns `true` if the document changed.
pub fn show(
    ui: &mut Ui,
    data: &mut Value,
    names: Names,
    unlocks: &mut GameProgress,
    spoilers: &mut super::spoilers::SpoilerUi<'_>,
) -> bool {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| contents(ui, data, names, unlocks, false, spoilers))
        .inner
}

/// Character tab contents without a scroll area. With `new_save`, choosing a
/// character also gives it the game's starting skills, as a new run would.
/// Returns `true` if the document changed.
pub fn contents(
    ui: &mut Ui,
    data: &mut Value,
    names: Names,
    unlocks: &mut GameProgress,
    new_save: bool,
    spoilers: &mut super::spoilers::SpoilerUi<'_>,
) -> bool {
    let mut changed = false;
    let Some(root) = data.as_object_mut() else {
        return false;
    };
    let original_character = root.get("character").cloned();
    section_heading(ui, names, "🐾", "Identity");
    let mut selected = None;
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            label(
                ui,
                names,
                "Character",
                "Playable character (`character`).",
            );
            selected = character_combo(ui, root, names, unlocks);
            let current = get_i64(root, "character").and_then(|c| usize::try_from(c).ok());
            if let Some(c) = current
                && unlocks.is_known()
                && !unlocks.is_unlocked(c)
            {
                ui.colored_label(colors::WARN, format!("🔒 {}", names.text("Locked in the game")))
                    .on_hover_text(names.character_unlock_requirement());
            }
            if !new_save
                && let Some(c) = current
                && c < schema::CHARACTERS.len()
                && ui
                    .button(names.text("Reset skills to starting values").to_string())
                    .on_hover_text(
                        names
                            .text("Set the levels and XP gain multipliers this character starts a run with.")
                            .to_string(),
                    )
                    .clicked()
                && let Some(skills) = root
                    .get_mut("body")
                    .and_then(|b| b.get_mut("skills"))
                    .and_then(Value::as_object_mut)
            {
                let mut updated = skills.clone();
                if super::operation_result(ui, model::setup_character_skills(&mut updated, c)) {
                    *skills = updated;
                    changed = true;
                }
            }
        });
        if let Some(c) = get_i64(root, "character").and_then(|c| usize::try_from(c).ok()) {
            character_info(ui, names, c);
        }
        let character = get_i64(root, "character").and_then(|c| usize::try_from(c).ok());
        for (index, &(key, text, help)) in IDENTITY.iter().enumerate() {
            let range = character.and_then(|c| schema::identity_range(index, c));
            if let Some(v) = root.get_mut(key) {
                ui.horizontal(|ui| {
                    label(ui, names, text, help);
                    changed |= number_editor(ui, v, 0.2);
                    if let Some(range) = range {
                        let text = format!(
                            "{} {}–{}",
                            names.text("game range:"),
                            range.start(),
                            range.end()
                        );
                        if v.as_i64().is_some_and(|value| !range.contains(&value)) {
                            ui.colored_label(colors::WARN, format!("⚠ {text}"));
                        } else {
                            ui.weak(text);
                        }
                    }
                });
            }
        }
    });
    if let Some(character) = selected {
        let mut accepted = true;
        if new_save
            && let Some(skills) = root
                .get_mut("body")
                .and_then(|b| b.get_mut("skills"))
                .and_then(Value::as_object_mut)
        {
            let mut updated = skills.clone();
            accepted =
                super::operation_result(ui, model::setup_character_skills(&mut updated, character));
            if accepted {
                *skills = updated;
            }
        }
        if accepted {
            if new_save && let Some(height) = save::roll_identity(0, character) {
                root.insert(schema::IDENTITY_KEYS[0].to_owned(), height.into());
            }
            changed = true;
        } else if let Some(original) = original_character {
            root.insert("character".to_owned(), original);
        }
    }
    unlocks_card(ui, names, unlocks);
    if !new_save {
        endings_card(ui, names, unlocks, spoilers);
    }

    section_heading(ui, names, "💪", "Skills");
    let Some(skills) = root
        .get_mut("body")
        .and_then(|b| b.get_mut("skills"))
        .and_then(Value::as_object_mut)
    else {
        ui.colored_label(
            colors::WARN,
            names.text("No `body.skills` in this save.").to_string(),
        );
        return changed;
    };
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        for def in &schema::SKILLS {
            changed |= skill_row(ui, skills, def, names);
            ui.add_space(6.0);
        }
        ui.weak(
            names
                .text("Changing a level recalculates the experience bounds exactly like the game does, keeping the progress towards the next level.")
                .to_string(),
        );
    });
    let known = |k: &str| {
        schema::SKILLS.iter().any(|s| {
            [
                s.key.to_owned(),
                format!("exp{}", s.key),
                format!("min{}", s.key),
                format!("max{}", s.key),
                format!("{}GainMult", s.key),
                format!("{}From10", s.key),
            ]
            .iter()
            .any(|x| x == k)
        })
    };
    changed |= super::other_fields(ui, "skills_other", skills, &known);
    changed
}

fn set_skill_level(skills: &mut Map<String, Value>, key: &str, level: i64) -> Result<(), String> {
    let level = i32::try_from(level)
        .ok()
        .filter(|level| *level >= 0)
        .ok_or_else(|| "Skill level is outside the game's integer range".to_owned())?;
    let next = level
        .checked_add(1)
        .ok_or_else(|| "The next skill level exceeds the game's integer range".to_owned())?;
    if schema::try_experience_for_level(level).is_none()
        || schema::try_experience_for_level(next).is_none()
    {
        return Err("Skill experience bounds are unavailable for this level".to_owned());
    }
    model::set_skill_level(skills, key, i64::from(level))
}

fn skill_row(
    ui: &mut Ui,
    skills: &mut Map<String, Value>,
    def: &schema::SkillDef,
    names: Names,
) -> bool {
    let mut changed = false;
    let key = def.key;
    let (level, progress) = model::skill_progress(skills, key);
    ui.horizontal(|ui| {
        ui.add_sized(
            [LABEL_WIDTH, 22.0],
            egui::Label::new(RichText::new(format!("{} ({key})", names.skill(def))).strong()),
        )
        .on_hover_text(names.text(def.help).to_string());
        ui.label(names.text("Level").to_string());
        let mut new_level = level;
        if ui
            .add(
                egui::DragValue::new(&mut new_level)
                    .range(0..=i64::from(i32::MAX - 1))
                    .speed(0.1),
            )
            .changed()
        {
            changed |= super::operation_result(ui, set_skill_level(skills, key, new_level));
        }
        for (txt, delta) in [("−", -1), ("+", 1)] {
            if ui.small_button(txt).clicked() {
                let new_level = level.saturating_add(delta).max(0);
                changed |= super::operation_result(ui, set_skill_level(skills, key, new_level));
            }
        }
        let min = get_f64(skills, &format!("min{key}")).unwrap_or(0.0);
        let max = get_f64(skills, &format!("max{key}")).unwrap_or(0.0);
        let exp = get_f64(skills, &format!("exp{key}")).unwrap_or(0.0);
        ui.add(
            ProgressBar::new(progress as f32)
                .desired_width(220.0)
                .fill(colors::NEUTRAL)
                .text(format!(
                    "{:.0} / {:.0} XP ({:.0}%)",
                    exp,
                    max,
                    progress * 100.0
                )),
        )
        .on_hover_text(
            names
                .text("Level {level}: {min} XP, level {next}: {max} XP")
                .replace("{level}", &level.to_string())
                .replace("{min}", &format!("{min:.0}"))
                .replace("{next}", &level.saturating_add(1).to_string())
                .replace("{max}", &format!("{max:.0}")),
        );
    });
    ui.horizontal(|ui| {
        ui.add_space(LABEL_WIDTH + ui.spacing().item_spacing.x);
        ui.label(names.text("Experience").to_string());
        let min = get_f64(skills, &format!("min{key}")).unwrap_or(0.0);
        let max = get_f64(skills, &format!("max{key}")).unwrap_or(0.0);
        if let Some(slot) = skills.get_mut(&format!("exp{key}")) {
            let mut v = slot.as_f64().unwrap_or(0.0);
            let range = min..=(max - 0.01).max(min);
            if ui
                .add(
                    egui::DragValue::new(&mut v)
                        .range(range)
                        .speed(1.0)
                        .max_decimals(2),
                )
                .on_hover_text(
                    names
                        .text("Experience within the current level")
                        .to_string(),
                )
                .changed()
            {
                crate::json::set_number_preserving(slot, v);
                changed = true;
            }
        }
        ui.label(names.text("Gain multiplier").to_string());
        if let Some(slot) = skills.get_mut(&format!("{key}GainMult")) {
            changed |= number_editor(ui, slot, 0.01);
        }
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_level_rejects_negative_and_unrepresentable_next_levels() {
        let mut skills = Map::new();
        assert!(set_skill_level(&mut skills, "STR", -1).is_err());
        assert!(set_skill_level(&mut skills, "STR", i64::from(i32::MAX)).is_err());
        assert!(set_skill_level(&mut skills, "STR", i64::MAX).is_err());
        assert!(skills.is_empty());
    }

    #[test]
    fn skill_level_above_99_uses_the_installed_curve() {
        let _metadata =
            crate::game_data::runtime::scoped_for_test(crate::game_data::metadata::Metadata {
                experience: Some(crate::game_data::program::ScalarProgram::test_linear(
                    None, 100.0, 0.0,
                )),
                ..Default::default()
            });
        let mut skills = Map::new();
        set_skill_level(&mut skills, "STR", 120).unwrap();
        assert_eq!(skills["STR"], 120);
        assert_eq!(skills["minSTR"], 12000);
        assert_eq!(skills["maxSTR"], 12100);
    }
}
