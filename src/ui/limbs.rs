//! Limbs tab: a painted body figure (like the in-game health screen) and an
//! editor for the selected limb.

use egui::{RichText, Ui};
use serde_json::Value;
use std::collections::HashMap;

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

fn localized_problems(limb: &Value, names: Names) -> String {
    limb_problems(limb)
        .into_iter()
        .map(|problem| names.text(problem))
        .collect::<Vec<_>>()
        .join(", ")
}

fn prosthetic_label(key: &str, names: Names) -> String {
    let source = match key {
        "hasProsthetic" => "Has prosthetic",
        "itemId" => "Prosthetic item ID",
        "forceChange" => "Prosthetic force change",
        "armorChange" => "Prosthetic armor change",
        "destroyItem" => "Item on destruction",
        "appearancePath" => "Prosthetic appearance path",
        "maxHoles" => "Maximum holes",
        "r" => "Prosthetic color: red",
        "g" => "Prosthetic color: green",
        "b" => "Prosthetic color: blue",
        _ => return key.to_owned(),
    };
    names.text(source).into_owned()
}

fn heal_button(ui: &mut Ui, data: &mut Value, label: &str, regrow: bool) -> bool {
    if ui.button(label).clicked() {
        return super::operation_result(ui, model::heal_limbs(data, regrow));
    }
    false
}

/// Shows the limbs tab. Returns `true` if the document changed.
pub fn show(
    ui: &mut Ui,
    data: &mut Value,
    names: Names,
    stats: &HashMap<String, crate::game_data::ItemStats>,
    state: &mut LimbsState,
) -> bool {
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
                    protection_panel(ui, data, names, stats, state);
                    ui.add_space(10.0);
                    changed |= limb_editor(ui, data, names, state);
                    ui.add_space(10.0);
                    limb_table(ui, data, names, state);
                });
            });
    });
    changed
}

fn protection_panel(
    ui: &mut Ui,
    data: &Value,
    names: Names,
    stats: &HashMap<String, crate::game_data::ItemStats>,
    state: &mut LimbsState,
) {
    egui::CollapsingHeader::new(names.text("Body protection"))
        .default_open(true)
        .show(ui, |ui| {
            let result = model::calculate_protection(data, stats, crate::game_data::runtime::metadata());
            let protection = match result {
                Ok(protection) => protection,
                Err(error) => {
                    ui.colored_label(colors::WARN, names.text("Protection calculation unavailable."));
                    ui.label(error);
                    return;
                }
            };
            ui.label(names.text("Armor is a damage divisor, not a separate resistance for each damage type. Only effects that use armor are reduced."));
            ui.label(format!("{}: {}", names.text("Whole-body clothing insulation"), super::fmt_num(f64::from(protection.clothing_insulation))));
            ui.weak(names.text("Insulation is global in the game, not calculated per limb. Other factors also affect total insulation."));
            match model::calculate_insulation(data, protection.clothing_insulation, crate::game_data::runtime::metadata()) {
                Ok([awake, sleeping]) => {
                    ui.label(format!("{}: {}%", names.text("Total insulation without sleeping bag"), super::fmt_num(f64::from(awake * 100.0))));
                    ui.label(format!("{}: {}%", names.text("Total insulation with sleeping bag"), super::fmt_num(f64::from(sleeping * 100.0))));
                    ui.weak(names.text("Sleeping-bag use is temporary and is not saved; both possibilities are shown."));
                }
                Err(error) => {
                    ui.colored_label(colors::WARN, names.text("Total insulation unavailable."));
                    ui.label(error);
                }
            }
            egui::Grid::new("limb_protection").striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
                ui.strong(names.text("Limb"));
                ui.strong(names.text("Armor divisor"));
                ui.strong(names.text("Damage reduction"));
                ui.strong(names.text("Affected damage"));
                ui.end_row();
                for (index, limb) in protection.limbs.iter().enumerate() {
                    if ui.selectable_label(state.selected == index, names.limb(index)).clicked() {
                        state.selected = index;
                    }
                    if data["limbs"][index]["dismembered"].as_bool() == Some(true) {
                        ui.weak("—");
                        ui.weak(names.text("Dismembered"));
                        ui.weak("—");
                    } else {
                        ui.label(format!("×{}", super::fmt_num(f64::from(limb.divisor))));
                        ui.label(format!("{}%", ((1.0_f32 - 1.0 / limb.divisor) * 100.0).round_ties_even()));
                        ui.label(format!("{}%", super::fmt_num(f64::from(100.0 / limb.divisor))));
                    }
                    ui.end_row();
                }
            });
            if let Some(limb) = protection.limbs.get(state.selected) {
                ui.add_space(5.0);
                ui.strong(names.text("Selected limb armor sources"));
                ui.label(format!("{}: {}", names.text("Run setting"), super::fmt_num(f64::from(protection.run_bonus))));
                ui.label(format!("{}: {}", names.text("Character bonus"), super::fmt_num(f64::from(protection.character_bonus))));
                ui.label(format!("{}: {}", names.text("Prosthetic"), super::fmt_num(f64::from(limb.prosthetic))));
                for (id, armor) in &limb.clothing {
                    ui.label(format!("{}: {}", names.item(id), super::fmt_num(f64::from(*armor))));
                }
            }
        });
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
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [super::LABEL_WIDTH, ui.spacing().interact_size.y],
                                egui::Label::new(prosthetic_label(&k, names)).truncate(),
                            )
                            .on_hover_text(names.text("JSON key: {key}").replace("{key}", &k));
                            changed |= super::raw::leaf_editor(ui, v);
                        });
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
                        let problems = localized_problems(limb, names);
                        if problems.is_empty() {
                            ui.colored_label(colors::GOOD, names.text("OK"));
                        } else {
                            ui.colored_label(colors::WARN, problems);
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
    #[allow(deprecated)]
    fn protection_panel_displays_armor_without_editing_the_save() {
        use crate::game_data::{metadata::Metadata, protection::ProtectionData, runtime};
        let _metadata = runtime::scoped_for_test(Metadata {
            limbs: vec![schema::LimbDef {
                object: "Head",
                name: "Head",
            }],
            hold_slots: vec![schema::HoldSlotDef {
                name: "Hand",
                locale_key: "",
                icon: "",
            }],
            protection: Some(ProtectionData {
                character_bonus: [(0, 0.0)].into(),
                base: 1.0,
                minimum: 0.1,
                ..Default::default()
            }),
            ..Default::default()
        });
        let data = json!({
            "character": 0, "runSettings": [{"Item1":"bonuslimbarmor", "Item2":"1"}],
            "limbs": [{"dismembered":false}], "items": []
        });
        let original = data.clone();
        let context = egui::Context::default();
        let names = Names {
            locale: None,
            interface_language: super::super::InterfaceLanguage::English,
        };
        let mut state = LimbsState::default();
        let mut output = context.run_ui(Default::default(), |ui| {
            protection_panel(ui, &data, names, &HashMap::new(), &mut state);
        });
        output.textures_delta.clear();
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    Some(text.galley.text())
                } else {
                    None
                }
            })
            .collect();
        assert!(labels.contains(&"Body protection"), "{labels:?}");
        assert!(labels.contains(&"×2"), "{labels:?}");
        assert!(labels.contains(&"50%"), "{labels:?}");
        assert_eq!(data, original);
    }

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

    #[test]
    fn limb_statuses_and_prosthetic_fields_use_interface_language() {
        use super::super::InterfaceLanguage;
        let limb = json!({
            "broken": true, "dislocated": true, "bleedAmount": 1,
            "infected": true, "pain": 21, "shrapnel": 1,
            "blockedBleeding": true, "splinted": true,
            "prostheticInfo": {"hasProsthetic": true}
        });
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::Russian,
        };
        assert_eq!(
            localized_problems(&limb, names),
            "перелом, вывих, кровотечение, инфекция, боль, осколки, жгут, наложена шина, протез"
        );
        assert_eq!(
            localized_problems(&json!({"dismembered": true}), names),
            "ампутирована"
        );
        for key in [
            "hasProsthetic",
            "itemId",
            "forceChange",
            "armorChange",
            "destroyItem",
            "appearancePath",
            "maxHoles",
            "r",
            "g",
            "b",
        ] {
            assert_ne!(prosthetic_label(key, names), key);
        }
        assert_eq!(prosthetic_label("futureField", names), "futureField");

        let pack = crate::interface_packs::InterfacePack {
            code: "custom".into(),
            name: "Custom".into(),
            translations: [
                ("broken".into(), "custom fracture".into()),
                ("Prosthetic armor change".into(), "custom armor".into()),
            ]
            .into(),
        };
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::External(&pack),
        };
        assert_eq!(
            localized_problems(&json!({"broken": true}), names),
            "custom fracture"
        );
        assert_eq!(prosthetic_label("armorChange", names), "custom armor");
    }

    #[test]
    #[allow(deprecated)]
    fn limb_table_and_prosthetic_editor_render_without_editing() {
        let _metadata = crate::game_data::runtime::scoped_for_test(
            crate::game_data::metadata::Metadata::default(),
        );
        let mut data = json!({"limbs": [
            {"broken": true, "prostheticInfo": {
                "hasProsthetic": true, "armorChange": 0.5
            }},
            {}
        ]});
        let original = data.clone();
        let names = Names {
            locale: None,
            interface_language: super::super::InterfaceLanguage::Russian,
        };
        let mut state = LimbsState::default();
        let context = egui::Context::default();
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1800.0, 1000.0),
                )),
                ..Default::default()
            },
            |ui| {
                assert!(!limb_editor(ui, &mut data, names, &mut state));
                limb_table(ui, &data, names, &mut state);
            },
        );
        output.textures_delta.clear();
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    Some(text.galley.text())
                } else {
                    None
                }
            })
            .collect();
        for expected in ["Изменение брони протезом", "перелом, протез", "В норме"]
        {
            assert!(labels.contains(&expected), "{expected}: {labels:?}");
        }
        assert_eq!(data, original);
    }
}
