//! Run tab: progress of the current run, run settings, mood history and
//! saved recipes.

use egui::{Color32, RichText, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Map, Value};

use super::{LABEL_WIDTH, Names, card, colors, number_editor, section_heading};
use crate::json::{self, get_bool};
use crate::schema::{self, RunSettingKind};

/// Formats seconds as `h:mm:ss`.
pub fn format_duration(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

fn label(ui: &mut Ui, names: Names, text: &str, help: &str) {
    ui.add_sized(
        [LABEL_WIDTH, ui.spacing().interact_size.y],
        egui::Label::new(names.text(text).to_string()),
    )
    .on_hover_text(help);
}

fn flag(
    ui: &mut Ui,
    names: Names,
    root: &mut Map<String, Value>,
    key: &str,
    text: &str,
    help: &str,
) -> bool {
    let Some(mut b) = get_bool(root, key) else {
        return false;
    };
    let mut changed = false;
    ui.horizontal(|ui| {
        label(ui, names, text, help);
        let text = names.text(if b { "Yes" } else { "No" }).to_string();
        if ui.checkbox(&mut b, text).changed() {
            json::set_bool(root, key, b);
            changed = true;
        }
    });
    changed
}

fn number(
    ui: &mut Ui,
    names: Names,
    root: &mut Map<String, Value>,
    key: &str,
    text: &str,
    help: &str,
    speed: f64,
) -> bool {
    let Some(slot) = root.get_mut(key) else {
        return false;
    };
    let mut changed = false;
    ui.horizontal(|ui| {
        label(ui, names, text, help);
        changed = number_editor(ui, slot, speed);
    });
    changed
}

/// Shows the run tab. Returns `true` if the document changed.
pub fn show(ui: &mut Ui, data: &mut Value, names: Names) -> bool {
    let mut changed = false;
    let Some(root) = data.as_object_mut() else {
        return false;
    };
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            section_heading(ui, names, "🗺", "Run progress");
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                if let Some(slot) = root.get_mut("biome") {
                    ui.horizontal(|ui| {
                        label(
                            ui,
                            names,
                            "Layer",
                            "Layer index the run resumes on (`biome`).",
                        );
                        changed |= number_editor(ui, slot, 0.05);
                        ui.label(
                            RichText::new(names.layer(slot.as_i64().unwrap_or(0)))
                                .color(colors::ACCENT),
                        );
                    });
                }
                if let Some(slot) = root.get_mut("runTime") {
                    ui.horizontal(|ui| {
                        label(
                            ui,
                            names,
                            "Run time (s)",
                            "Total time of the run in seconds.",
                        );
                        changed |= number_editor(ui, slot, 1.0);
                        ui.weak(format_duration(slot.as_f64().unwrap_or(0.0)));
                    });
                }
                changed |= number(
                    ui,
                    names,
                    root,
                    "totalTraveled",
                    "Distance traveled",
                    "Total distance traveled in the run.",
                    1.0,
                );
                changed |= number(
                    ui,
                    names,
                    root,
                    "endlessLoops",
                    "Endless loops",
                    "Number of completed endless-mode loops.",
                    0.05,
                );
                changed |= number(
                    ui,
                    names,
                    root,
                    "caloriesConsumed",
                    "Calories consumed",
                    "Calories eaten during the run.",
                    1.0,
                );
                changed |= number(
                    ui,
                    names,
                    root,
                    "trapRarity",
                    "Trap rarity",
                    "Trap rarity multiplier.",
                    0.01,
                );
                changed |= number(
                    ui,
                    names,
                    root,
                    "lootRarity",
                    "Loot rarity",
                    "Loot rarity multiplier.",
                    0.01,
                );
                changed |= flag(
                    ui,
                    names,
                    root,
                    "isMercRun",
                    "Mercenary run",
                    "The run was started as a mercenary.",
                );
                changed |= flag(
                    ui,
                    names,
                    root,
                    "cheatedSave",
                    "Cheated save",
                    "Marks the save as cheated; achievements are disabled.",
                );
                if get_bool(root, "cheatedSave") == Some(true) {
                    ui.colored_label(
                        colors::WARN,
                        "⚠ Achievements are disabled for cheated saves.",
                    );
                }
            });

            section_heading(ui, names, "⚙", "Run settings");
            if let Some(Value::Array(settings)) = root.get_mut("runSettings") {
                changed |= run_settings(ui, settings, names);
            } else {
                ui.weak(names.text("No run settings in this save.").to_string());
            }

            section_heading(ui, names, "🙂", "Mood history");
            if let Some(Value::Array(mood)) = root.get_mut("lastHappiness") {
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    changed |= mood_chart(ui, mood, names);
                });
            }

            if let Some(Value::Array(saved)) = root
                .get_mut("savedRecipeData")
                .and_then(|r| r.get_mut("saved"))
            {
                ui.add_space(6.0);
                changed |= recipes(ui, saved, names);
            }
        });
    changed
}

fn setting_editor(ui: &mut Ui, id: &str, slot: &mut Value, names: Names) -> bool {
    let Some(def) = schema::run_setting(id) else {
        return super::raw::leaf_editor(ui, slot);
    };
    match def.kind {
        RunSettingKind::Bool => {
            let mut b = slot
                .as_bool()
                .unwrap_or_else(|| slot.as_str() == Some("True"));
            let changed = ui.checkbox(&mut b, "").changed();
            if changed {
                *slot = Value::Bool(b);
            }
            changed
        }
        RunSettingKind::Float { min, max, whole } => {
            let mut v = slot.as_f64().unwrap_or(0.0);
            let mut slider = egui::Slider::new(&mut v, min as f64..=max as f64)
                .clamping(egui::SliderClamping::Never)
                .suffix(def.postfix);
            if whole {
                slider = slider.integer();
            } else {
                slider = slider.max_decimals(2).step_by(0.01);
            }
            let changed = ui.add(slider).changed();
            if changed {
                // The game parses the value with float.Parse, so store a float.
                *slot = Value::Number(json::f32_number(v));
            }
            changed
        }
        RunSettingKind::Dropdown(_) => {
            let current = slot.as_i64().unwrap_or(0);
            let mut value = current;
            let count = match def.kind {
                RunSettingKind::Dropdown(o) => o.len(),
                _ => 0,
            };
            egui::ComboBox::from_id_salt(("runset", id))
                .selected_text(names.text(&names.run_setting_option(id, value as usize)))
                .show_ui(ui, |ui| {
                    for i in 0..count {
                        ui.selectable_value(
                            &mut value,
                            i as i64,
                            names.text(&names.run_setting_option(id, i)),
                        );
                    }
                });
            if value != current {
                // Must stay an integer: the game uses int.Parse.
                *slot = Value::from(value);
                true
            } else {
                false
            }
        }
    }
}

fn run_settings(ui: &mut Ui, settings: &mut [Value], names: Names) -> bool {
    let mut changed = false;
    let mut sections: Vec<&str> = Vec::new();
    for def in schema::RUN_SETTINGS {
        if !sections.contains(&def.section) {
            sections.push(def.section);
        }
    }
    sections.push("Other");
    let section_of = |id: &str| schema::run_setting(id).map_or("Other", |d| d.section);
    for section in sections {
        let members: Vec<usize> = settings
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                section_of(s.get("Item1").and_then(Value::as_str).unwrap_or("")) == section
            })
            .map(|(i, _)| i)
            .collect();
        if members.is_empty() {
            continue;
        }
        egui::CollapsingHeader::new(
            RichText::new(format!("{} ({})", names.text(section), members.len())).strong(),
        )
        .id_salt(("runsection", section))
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new(("runsettings", section))
                .striped(true)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for i in members {
                        let Some(obj) = settings[i].as_object_mut() else {
                            continue;
                        };
                        let id = obj
                            .get("Item1")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned();
                        let mut resp = ui.add_sized(
                            [LABEL_WIDTH + 60.0, ui.spacing().interact_size.y],
                            egui::Label::new(names.text(&names.run_setting(&id)).to_string())
                                .truncate(),
                        );
                        if let Some(desc) = names.run_setting_description(&id) {
                            resp = resp.on_hover_text(desc);
                        }
                        resp.on_hover_text(format!("id: {id}"));
                        if let Some(slot) = obj.get_mut("Item2") {
                            changed |= setting_editor(ui, &id, slot, names);
                        }
                        ui.end_row();
                    }
                });
        });
    }
    changed
}

/// Shows the game's run-setting controls for a new-save wizard.
pub fn run_settings_editor(ui: &mut Ui, settings: &mut [Value], names: Names) -> bool {
    run_settings(ui, settings, names)
}

/// Line chart of `lastHappiness` with editable points (drag a point
/// vertically to change it).
fn mood_chart(ui: &mut Ui, mood: &mut [Value], names: Names) -> bool {
    let mut changed = false;
    let values: Vec<f64> = mood.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect();
    let extent = values.iter().fold(5.0f64, |m, v| m.max(v.abs() * 1.2));
    let width = ui.available_width().min(700.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 140.0), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    let n = values.len().max(2);
    let x_of = |i: usize| rect.left() + 12.0 + (rect.width() - 24.0) * i as f32 / (n - 1) as f32;
    let y_of = |v: f64| rect.center().y - (v / extent) as f32 * (rect.height() / 2.0 - 10.0);
    painter.line_segment(
        [
            pos2(rect.left(), rect.center().y),
            pos2(rect.right(), rect.center().y),
        ],
        Stroke::new(1.0, ui.visuals().weak_text_color()),
    );
    let points: Vec<_> = values
        .iter()
        .enumerate()
        .map(|(i, &v)| pos2(x_of(i), y_of(v)))
        .collect();
    painter.add(egui::Shape::line(
        points.clone(),
        Stroke::new(2.0, colors::ACCENT),
    ));
    for (p, &v) in points.iter().zip(&values) {
        let c = if v >= 0.0 { colors::GOOD } else { colors::BAD };
        painter.circle(*p, 4.0, c, Stroke::new(1.0, Color32::BLACK));
    }
    painter.text(
        rect.left_top() + vec2(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        format!("+{}", super::fmt_num(extent)),
        egui::FontId::proportional(11.0),
        ui.visuals().weak_text_color(),
    );
    painter.text(
        rect.left_bottom() + vec2(4.0, -2.0),
        egui::Align2::LEFT_BOTTOM,
        format!("-{}", super::fmt_num(extent)),
        egui::FontId::proportional(11.0),
        ui.visuals().weak_text_color(),
    );
    if let Some(pos) = response.interact_pointer_pos()
        && response.is_pointer_button_down_on()
        && !values.is_empty()
    {
        let i = ((pos.x - rect.left() - 12.0) / (rect.width() - 24.0) * (n - 1) as f32)
            .round()
            .clamp(0.0, (values.len() - 1) as f32) as usize;
        let v = -((pos.y - rect.center().y) / (rect.height() / 2.0 - 10.0)) as f64 * extent;
        let v = (v * 100.0).round() / 100.0;
        json::set_number_preserving(&mut mood[i], v);
        changed = true;
    }
    response.on_hover_text(
        names
            .text("Recent mood samples (oldest → newest). Drag points to edit.")
            .to_string(),
    );
    ui.horizontal_wrapped(|ui| {
        for slot in mood.iter_mut() {
            changed |= number_editor(ui, slot, 0.05);
        }
    });
    changed
}

fn recipes(ui: &mut Ui, saved: &mut [Value], names: Names) -> bool {
    let mut changed = false;
    let known = saved
        .iter()
        .filter(|r| r.get("Item1").and_then(Value::as_bool) == Some(true))
        .count();
    egui::CollapsingHeader::new(
        RichText::new(format!(
            "📜 {} ({known} / {} {})",
            names.text("Saved recipe data"),
            saved.len(),
            names.text("marked")
        ))
        .strong(),
    )
    .id_salt("recipes")
    .show(ui, |ui| {
        ui.weak(
            names
                .text("`savedRecipeData.saved`: per-recipe flag (Item1) and value (Item2), in game order.")
                .to_string(),
        );
        ui.horizontal(|ui| {
            for (text, value) in [("Mark all", true), ("Unmark all", false)] {
                if ui.button(names.text(text).to_string()).clicked() {
                    for r in saved.iter_mut().filter_map(Value::as_object_mut) {
                        json::set_bool(r, "Item1", value);
                    }
                    changed = true;
                }
            }
        });
        egui::Grid::new("recipes_grid")
            .striped(true)
            .show(ui, |ui| {
                for (i, r) in saved.iter_mut().enumerate() {
                    let Some(obj) = r.as_object_mut() else {
                        continue;
                    };
                    ui.weak(format!("#{i}"));
                    let mut b = get_bool(obj, "Item1").unwrap_or(false);
                    if ui.checkbox(&mut b, "").changed() {
                        json::set_bool(obj, "Item1", b);
                        changed = true;
                    }
                    if let Some(slot) = obj.get_mut("Item2") {
                        changed |= number_editor(ui, slot, 0.1);
                    }
                    if i % 4 == 3 {
                        ui.end_row();
                    } else {
                        ui.separator();
                    }
                }
            });
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_is_formatted() {
        assert_eq!(format_duration(12815.7324), "3:33:36");
        assert_eq!(format_duration(59.4), "0:00:59");
    }
}
