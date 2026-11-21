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
                    section_heading(ui, names, "🧍", "Body");
                    card(ui, |ui| {
                        result.open_limb = super::limbs::small_figure(ui, data, names, 360.0);
                        ui.weak(names.text("Click a limb to edit it.").to_string());
                    });
                });
                ui.vertical(|ui| {
                    summary(ui, data, names);
                    section_heading(ui, names, "❤", "Vital signs");
                    card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        vitals(ui, data, names);
                    });
                    section_heading(ui, names, "⚡", "Quick fixes");
                    card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        result.changed |= quick_fixes(ui, data, names);
                    });
                });
            });
        });
    result
}

fn summary(ui: &mut Ui, data: &Value, names: Names) {
    let Some(root) = data.as_object() else { return };
    section_heading(ui, names, "🐾", "Run");
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        let character = get_i64(root, "character").map_or_else(
            || names.text("Unknown").into_owned(),
            |c| names.character(c),
        );
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(character).heading().strong());
            if let Some(id) = get_i64(root, "cId") {
                ui.label(RichText::new(format!("#{id}")).heading().weak());
            }
            ui.separator();
            if let Some(age) = get_i64(root, "cAge") {
                ui.label(format!("{} {age}", names.text("Age")));
            }
            if let Some(h) = get_i64(root, "cHeight") {
                ui.label(format!("· {h} {}", names.text("cm")));
            }
        });
        ui.horizontal_wrapped(|ui| {
            if let Some(b) = get_i64(root, "biome") {
                ui.label(names.text("Layer:").to_string());
                ui.label(RichText::new(names.layer(b)).color(colors::ACCENT).strong());
                ui.weak(format!("({} {b})", names.text("Biome")));
            }
            if let Some(t) = get_f64(root, "runTime") {
                ui.separator();
                ui.label(format!("⏱ {}", super::run::format_duration(t)));
            }
            if let Some(d) = get_i64(root, "totalTraveled") {
                ui.separator();
                ui.label(format!("🚶 {d} {}", names.text("m traveled")));
            }
            let items = root
                .get("items")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            ui.separator();
            ui.label(format!("🎒 {items} {}", names.text("items")));
            if get_bool(root, "cheatedSave") == Some(true) {
                ui.separator();
                ui.colored_label(colors::WARN, format!("⚠ {}", names.text("cheated save")));
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

fn vitals(ui: &mut Ui, data: &Value, names: Names) {
    let Some(body) = data.get("body").and_then(Value::as_object) else {
        ui.weak(names.text("No body data.").to_string());
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
                ui.label(names.field(spec)).on_hover_text(format!(
                    "{}\n{}",
                    names.text(spec.help),
                    names.text("JSON key: {key}").replace("{key}", spec.key),
                ));
                super::mini_gauge(ui, spec, v, 120.0, names);
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

fn quick_fixes(ui: &mut Ui, data: &mut Value, names: Names) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(format!("✚ {}", names.text("Heal limbs")))
            .on_hover_text(
                names
                    .text("Heal all limbs (keeps dismembered limbs missing)")
                    .to_string(),
            )
            .clicked()
        {
            changed |= super::operation_result(ui, model::heal_limbs(data, false));
        }
        if ui
            .button(format!("✚ {}", names.text("Heal + regrow limbs")))
            .clicked()
        {
            changed |= super::operation_result(ui, model::heal_limbs(data, true));
        }
        if ui
            .button(format!("❤ {}", names.text("Restore vitals")))
            .clicked()
        {
            changed |= super::operation_result(ui, model::restore_vitals(data));
        }
        if ui
            .button(format!("🍖 {}", names.text("Fill needs")))
            .clicked()
        {
            changed |= super::operation_result(ui, model::fill_needs(data));
        }
        if ui
            .button(format!("🌟 {}", names.text("Full recovery")))
            .on_hover_text(names.text("All of the above").to_string())
            .clicked()
        {
            let mut recovered = data.clone();
            let result = model::heal_limbs(&mut recovered, true)
                .and_then(|()| model::restore_vitals(&mut recovered))
                .and_then(|()| model::fill_needs(&mut recovered));
            if super::operation_result(ui, result) {
                *data = recovered;
                changed = true;
            }
        }
    });
    ui.weak(
        names
            .text("Every change can be undone with Ctrl+Z; nothing is written until you save.")
            .to_string(),
    );
    changed
}
