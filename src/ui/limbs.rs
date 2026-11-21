//! Limbs tab: a painted body figure (like the in-game health screen) and an
//! editor for the selected limb.

use egui::{RichText, Ui};
use serde_json::Value;

use super::{Names, colors, field_row, other_fields};
use crate::json::{get_bool, get_f64};
use crate::{model, schema};

mod figure;
mod geometry;
pub use figure::{body_figure, legend, small_figure};

/// UI state of the limbs tab.
#[derive(Default)]
pub struct LimbsState {
    /// Index of the selected limb.
    pub selected: usize,
}

/// Overall condition (`0..1`) of a limb: the worse of skin and muscle health.
pub fn limb_condition(limb: &Value) -> f32 {
    let Some(obj) = limb.as_object() else {
        return 0.0;
    };
    let skin = get_f64(obj, "skinHealth").unwrap_or(100.0);
    let muscle = get_f64(obj, "muscleHealth").unwrap_or(100.0);
    (skin.min(muscle) / 100.0).clamp(0.0, 1.0) as f32
}

/// One-line list of the limb's problems (empty if healthy).
pub fn limb_problems(limb: &Value) -> Vec<&'static str> {
    let Some(obj) = limb.as_object() else {
        return Vec::new();
    };
    let flag = |k| get_bool(obj, k).unwrap_or(false);
    let num = |k| get_f64(obj, k).unwrap_or(0.0);
    let mut out = Vec::new();
    if flag("dismembered") {
        out.push("dismembered");
        return out;
    }
    if flag("broken") {
        out.push("broken");
    }
    if flag("dislocated") {
        out.push("dislocated");
    }
    if num("bleedAmount") > 0.0 {
        out.push("bleeding");
    }
    if flag("infected") || num("infectionAmount") > 0.0 {
        out.push("infected");
    }
    if num("pain") > 20.0 {
        out.push("in pain");
    }
    if obj.get("shrapnel").and_then(Value::as_i64).unwrap_or(0) > 0 {
        out.push("shrapnel");
    }
    if flag("blockedBleeding") {
        out.push("tourniquet");
    }
    if flag("splinted") {
        out.push("splinted");
    }
    if obj
        .get("prostheticInfo")
        .and_then(|p| p.get("hasProsthetic"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        out.push("prosthetic");
    }
    out
}

fn heal_button(ui: &mut Ui, data: &mut Value, label: &str, regrow: bool) -> bool {
    if ui.button(label).clicked() {
        return super::operation_result(ui, model::heal_limbs(data, regrow));
    }
    false
}

/// Shows the limbs tab. Returns `true` if the document changed.
pub fn show(ui: &mut Ui, data: &mut Value, names: Names, state: &mut LimbsState) -> bool {
    let mut changed = false;
    let Some(limbs) = data.get("limbs").and_then(Value::as_array).cloned() else {
        ui.colored_label(
            colors::BAD,
            names.text("This save has no `limbs` array.").to_string(),
        );
        return false;
    };
    state.selected = state.selected.min(limbs.len().saturating_sub(1));
    ui.horizontal(|ui| {
        changed |= heal_button(
            ui,
            data,
            &format!("✚ {}", names.text("Heal all limbs")),
            false,
        );
        changed |= heal_button(
            ui,
            data,
            &format!("✚ {}", names.text("Heal all + regrow dismembered")),
            true,
        );
    });
    ui.separator();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(220.0);
            if let Some(i) = body_figure(ui, &limbs, names, Some(state.selected), 420.0) {
                state.selected = i;
            }
            ui.add_space(6.0);
            legend(ui, names);
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    changed |= limb_editor(ui, data, names, state);
                    ui.add_space(10.0);
                    limb_table(ui, data, names, state);
                });
            });
    });
    changed
}

fn limb_editor(ui: &mut Ui, data: &mut Value, names: Names, state: &mut LimbsState) -> bool {
    let mut changed = false;
    let count = data
        .get("limbs")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("limb_select")
            .selected_text(RichText::new(names.limb(state.selected)).heading())
            .width(220.0)
            .show_ui(ui, |ui| {
                for i in 0..count {
                    ui.selectable_value(&mut state.selected, i, names.limb(i));
                }
            });
        ui.weak(format!(
            "limbs[{}] · {}",
            state.selected,
            schema::LIMBS.get(state.selected).map_or("?", |d| d.object)
        ));
    });
    let Some(limb) = data
        .get_mut("limbs")
        .and_then(|l| l.get_mut(state.selected))
        .and_then(Value::as_object_mut)
    else {
        return false;
    };
    ui.horizontal(|ui| {
        if ui
            .button(format!("✚ {}", names.text("Heal this limb")))
            .clicked()
        {
            changed |= super::operation_result(ui, model::heal_limb(limb, true));
        }
    });
    super::card(ui, |ui| {
        ui.set_width(ui.available_width());
        for spec in schema::LIMB_FIELDS {
            changed |= field_row(ui, spec, limb, 240.0, names);
        }
    });
    if let Some(Value::Object(p)) = limb.get_mut("prostheticInfo") {
        ui.add_space(6.0);
        egui::CollapsingHeader::new(format!("🦾 {}", names.text("Prosthetic")))
            .id_salt("prosthetic")
            .default_open(p.get("hasProsthetic").and_then(Value::as_bool) == Some(true))
            .show(ui, |ui| {
                let keys: Vec<String> = p.keys().cloned().collect();
                for k in keys {
                    if let Some(v) = p.get_mut(&k) {
                        changed |= super::generic_row(ui, &k, v);
                    }
                }
                ui.weak(
                    names
                        .text("Prosthetic item ids: peglimb, steelprosthesis, musharm …")
                        .to_string(),
                );
            });
    }
    let known = |k: &str| schema::limb_field(k).is_some() || k == "prostheticInfo";
    changed |= other_fields(ui, &format!("limb{}", state.selected), limb, &known);
    changed
}

fn limb_table(ui: &mut Ui, data: &Value, names: Names, state: &mut LimbsState) {
    let Some(limbs) = data.get("limbs").and_then(Value::as_array) else {
        return;
    };
    egui::CollapsingHeader::new(names.text("All limbs overview").to_string())
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new("limb_table")
                .striped(true)
                .spacing([14.0, 4.0])
                .show(ui, |ui| {
                    for h in [
                        "Limb",
                        "Skin",
                        "Muscle",
                        "Pain",
                        "Bleeding",
                        "Infection",
                        "Status",
                    ] {
                        ui.strong(names.text(h).to_string());
                    }
                    ui.end_row();
                    for (i, limb) in limbs.iter().enumerate() {
                        if ui
                            .selectable_label(state.selected == i, names.limb(i))
                            .clicked()
                        {
                            state.selected = i;
                        }
                        let obj = limb.as_object();
                        for key in [
                            "skinHealth",
                            "muscleHealth",
                            "pain",
                            "bleedAmount",
                            "infectionAmount",
                        ] {
                            let v = obj.and_then(|o| get_f64(o, key));
                            match (schema::limb_field(key), v) {
                                (Some(spec), Some(v)) => {
                                    super::mini_gauge(ui, spec, v, 60.0, names);
                                }
                                _ => {
                                    ui.weak("—");
                                }
                            }
                        }
                        let p = limb_problems(limb);
                        if p.is_empty() {
                            ui.colored_label(colors::GOOD, "OK");
                        } else {
                            ui.colored_label(colors::WARN, p.join(", "));
                        }
                        ui.end_row();
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn problems_are_listed() {
        let limb =
            json!({"skinHealth": 50.0, "muscleHealth": 80.0, "broken": true, "bleedAmount": 3.0});
        assert_eq!(limb_problems(&limb), vec!["broken", "bleeding"]);
        assert!((limb_condition(&limb) - 0.5).abs() < 1e-6);
        assert_eq!(
            limb_problems(&json!({"dismembered": true, "broken": true})),
            vec!["dismembered"]
        );
    }
}
