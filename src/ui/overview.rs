//! Overview tab: a dashboard of the most important values.

use egui::{ProgressBar, RichText, Ui};
use serde_json::Value;

use super::{Names, card, colors, fmt_num, section_heading, value_color};
use crate::json::{get_bool, get_f64, get_i64};
use crate::{model, schema};

/// Body fields shown on the dashboard.
const VITALS: &[&str] = &[
    "bloodVolume",
    "bloodOxygen",
    "heartRate",
    "bloodPressure",
    "consciousness",
    "brainHealth",
    "temperature",
    "happiness",
    "hunger",
    "thirst",
    "stamina",
    "energy",
    "shock",
    "sicknessAmount",
    "septicShock",
    "radiationSickness",
];

/// Result of drawing the overview.
#[derive(Default)]
pub struct OverviewResult {
    /// Whether the document changed.
    pub changed: bool,
    /// A limb was clicked: open it on the limbs tab.
    pub open_limb: Option<usize>,
}

/// Shows the overview tab.
pub fn show(ui: &mut Ui, data: &mut Value, names: Names) -> OverviewResult {
    let mut result = OverviewResult::default();
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(230.0);
                    section_heading(ui, "🧍", "Body");
                    card(ui, |ui| {
                        result.open_limb = super::limbs::small_figure(ui, data, names, 360.0);
                        ui.weak("Click a limb to edit it.");
                    });
                });
                ui.vertical(|ui| {
                    summary(ui, data, names);
                    section_heading(ui, "❤", "Vital signs");
                    card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        vitals(ui, data);
                    });
                    section_heading(ui, "⚡", "Quick fixes");
                    card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        result.changed |= quick_fixes(ui, data);
                    });
                });
            });
        });
    result
}

fn summary(ui: &mut Ui, data: &Value, names: Names) {
    let Some(root) = data.as_object() else { return };
    section_heading(ui, "🐾", "Run");
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        let character = get_i64(root, "character")
            .and_then(|c| usize::try_from(c).ok())
            .and_then(|c| schema::CHARACTERS.get(c))
            .copied()
            .unwrap_or("Unknown");
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(character).heading().strong());
            if let Some(id) = get_i64(root, "cId") {
                ui.label(RichText::new(format!("#{id}")).heading().weak());
            }
            ui.separator();
            if let Some(age) = get_i64(root, "cAge") {
                ui.label(format!("Age {age}"));
            }
            if let Some(h) = get_i64(root, "cHeight") {
                ui.label(format!("· {h} cm"));
            }
        });
        ui.horizontal_wrapped(|ui| {
            if let Some(b) = get_i64(root, "biome") {
                ui.label("Layer:");
                ui.label(RichText::new(names.layer(b)).color(colors::ACCENT).strong());
                ui.weak(format!("(biome {b})"));
            }
            if let Some(t) = get_f64(root, "runTime") {
                ui.separator();
                ui.label(format!("⏱ {}", super::run::format_duration(t)));
            }
            if let Some(d) = get_i64(root, "totalTraveled") {
                ui.separator();
                ui.label(format!("🚶 {d} m traveled"));
            }
            let items = root
                .get("items")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            ui.separator();
            ui.label(format!("🎒 {items} items"));
            if get_bool(root, "cheatedSave") == Some(true) {
                ui.separator();
                ui.colored_label(colors::WARN, "⚠ cheated save");
            }
        });
        if let Some(skills) = root
            .get("body")
            .and_then(|b| b.get("skills"))
            .and_then(Value::as_object)
        {
            ui.horizontal_wrapped(|ui| {
                for def in &schema::SKILLS {
                    let (level, progress) = model::skill_progress(skills, def.key);
                    ui.label(RichText::new(format!("{} {level}", names.skill(def))).strong());
                    ui.add(
                        ProgressBar::new(progress as f32)
                            .desired_width(90.0)
                            .fill(colors::NEUTRAL),
                    )
                    .on_hover_text(format!(
                        "{:.0}% to level {}",
                        progress * 100.0,
                        level + 1
                    ));
                    ui.add_space(8.0);
                }
            });
        }
    });
}

fn vitals(ui: &mut Ui, data: &Value) {
    let Some(body) = data.get("body").and_then(Value::as_object) else {
        ui.weak("No body data.");
        return;
    };
    let columns = ((ui.available_width() / 330.0).floor() as usize).clamp(1, 3);
    egui::Grid::new("vitals")
        .num_columns(columns * 3)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            let mut col = 0;
            for key in VITALS {
                let (Some(spec), Some(v)) = (schema::body_field(key), get_f64(body, key)) else {
                    continue;
                };
                ui.label(spec.label).on_hover_text(spec.help);
                super::mini_gauge(ui, spec, v, 120.0);
                ui.colored_label(
                    value_color(spec, v as f32),
                    format!("{}{}", fmt_num(v), spec.unit),
                );
                col += 1;
                if col % columns == 0 {
                    ui.end_row();
                }
            }
        });
}

fn quick_fixes(ui: &mut Ui, data: &mut Value) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("✚ Heal limbs")
            .on_hover_text("Heal all limbs (keeps dismembered limbs missing)")
            .clicked()
        {
            model::heal_limbs(data, false);
            changed = true;
        }
        if ui.button("✚ Heal + regrow limbs").clicked() {
            model::heal_limbs(data, true);
            changed = true;
        }
        if ui.button("❤ Restore vitals").clicked() {
            model::restore_vitals(data);
            changed = true;
        }
        if ui.button("🍖 Fill needs").clicked() {
            model::fill_needs(data);
            changed = true;
        }
        if ui
            .button("🌟 Full recovery")
            .on_hover_text("All of the above")
            .clicked()
        {
            model::heal_limbs(data, true);
            model::restore_vitals(data);
            model::fill_needs(data);
            changed = true;
        }
    });
    ui.weak("Every change can be undone with Ctrl+Z; nothing is written until you save.");
    changed
}
