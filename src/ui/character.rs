//! Character tab: identity of the experiment and its skills.

use egui::{ProgressBar, RichText, Ui};
use serde_json::{Map, Value};

use super::{LABEL_WIDTH, Names, card, colors, number_editor, section_heading};
use crate::json::{get_f64, get_i64, set_i64};
use crate::{model, schema};

/// Top-level identity keys shown on this tab.
const IDENTITY: &[(&str, &str, &str)] = &[
    ("cHeight", "Height (cm)", "Character height."),
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

/// Character selector for the `character` field.
pub fn character_combo(ui: &mut Ui, root: &mut Map<String, Value>) -> bool {
    let Some(current) = get_i64(root, "character") else {
        ui.weak("—");
        return false;
    };
    let mut value = current;
    let name = |i: i64| {
        usize::try_from(i)
            .ok()
            .and_then(|i| schema::CHARACTERS.get(i))
            .map_or_else(|| format!("Character #{i}"), |s| (*s).to_owned())
    };
    egui::ComboBox::from_id_salt("character")
        .selected_text(name(value))
        .show_ui(ui, |ui| {
            for i in 0..schema::CHARACTERS.len() as i64 {
                ui.selectable_value(&mut value, i, name(i));
            }
        });
    if value != current {
        set_i64(root, "character", value);
        true
    } else {
        false
    }
}

/// Shows the character tab. Returns `true` if the document changed.
pub fn show(ui: &mut Ui, data: &mut Value, names: Names) -> bool {
    let mut changed = false;
    let Some(root) = data.as_object_mut() else {
        return false;
    };
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        section_heading(ui, names, "🐾", "Identity");
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                label(
                    ui,
                    names,
                    "Character",
                    "Playable character (`character`).",
                );
                changed |= character_combo(ui, root);
            });
            for &(key, text, help) in IDENTITY {
                if let Some(v) = root.get_mut(key) {
                    ui.horizontal(|ui| {
                        label(ui, names, text, help);
                        changed |= number_editor(ui, v, 0.2);
                    });
                }
            }
        });

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
            return;
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
    });
    changed
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
                    .range(0..=99)
                    .speed(0.1),
            )
            .changed()
        {
            model::set_skill_level(skills, key, new_level);
            changed = true;
        }
        for (txt, delta) in [("−", -1), ("+", 1)] {
            if ui.small_button(txt).clicked() {
                model::set_skill_level(skills, key, (level + delta).max(0));
                changed = true;
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
                .replace("{next}", &(level + 1).to_string())
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
