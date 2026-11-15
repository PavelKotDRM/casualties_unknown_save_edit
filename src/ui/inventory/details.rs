use std::collections::HashMap;

use egui::{ProgressBar, RichText, Ui};
use serde_json::{Map, Value, json};

use super::{
    InventoryState, Names, card, category_icon, colors, component_short_name, item_wear_slot,
    items, location_combo, row_label,
};
use crate::catalog;
use crate::game_data::ItemStats;
use crate::json::{self, get_bool, get_f64, get_str};
use crate::model::{self, Location};
use crate::ui::{fmt_num, gauge_bar, generic_row, number_editor, other_fields, raw};

pub(super) fn details(
    ui: &mut Ui,
    data: &mut Value,
    names: Names,
    stats: &HashMap<String, ItemStats>,
    state: &mut InventoryState,
    index: usize,
) -> bool {
    let mut changed = false;
    let Some(item) = items(data).get(index).cloned() else {
        return false;
    };
    let id = item
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let def = catalog::item(&id);
    ui.heading(format!(
        "{} {}",
        def.map_or("❓", |d| category_icon(d.category)),
        names.item(&id)
    ));
    if let Some(d) = names.item_description(&id) {
        ui.label(RichText::new(d).italics());
    }
    if let Some(d) = def {
        ui.weak(format!(
            "{} {}{}{}",
            names.text("Category:"),
            names.text(d.category),
            if d.wear_slot.is_empty() {
                String::new()
            } else {
                format!(" · {} ({})", names.text("wearable"), d.wear_slot)
            },
            if d.capacity > 0.0 {
                format!(" · holds {} ml", fmt_num(d.capacity as f64))
            } else {
                String::new()
            },
        ));
    }
    let weights = model::item_weights(data, index, stats);
    if let Some(item_stats) = stats.get(&id) {
        if let Some(game_weight) = weights.game {
            ui.weak(format!(
                "{} {:.2} kg{}{}",
                names.text("Game item weight:"),
                game_weight,
                item_stats.armor.map_or(String::new(), |value| {
                    format!(" · {}: {value:.2}", names.text("armor"))
                }),
                item_stats.insulation.map_or(String::new(), |value| {
                    format!(" · {}: {value:.2}", names.text("insulation"))
                }),
            ));
        } else {
            ui.colored_label(
                colors::WARN,
                names.text("Item weight unavailable.").to_string(),
            );
        }
        if item_stats.liquid_capacity.is_some() {
            if let (Some(liquid), Some(physical)) = (weights.liquid, weights.physical) {
                ui.weak(format!(
                    "{} ≈{:.2} kg · {} ≈{liquid:.2} kg · {} ≈{physical:.2} kg",
                    names.text("Empty container:"),
                    item_stats.total_weight(0.0),
                    names.text("Liquid:"),
                    names.text("Physical weight:"),
                )).on_hover_text(names.text("Approximation: liquids use 1 kg per litre; empty container weight is inferred from the game's minimum item weight. Container encumbrance modifiers do not apply.").to_string());
            } else {
                ui.colored_label(colors::WARN, names.text("Liquid contents are missing or invalid; weight estimates are incomplete.").to_string());
            }
        }
    } else {
        ui.colored_label(
            colors::WARN,
            names
                .text("Item stats unavailable; sync game data to show verified weight.")
                .to_string(),
        );
    }
    ui.add_space(6.0);

    // Location (moving changes the index).
    let mut loc = Location::of(&item);
    ui.horizontal(|ui| {
        row_label(ui, names, "Location");
        if location_combo(
            ui,
            "item_location",
            data,
            &mut loc,
            item_wear_slot(&id, stats),
            true,
            names.interface_language,
        ) && let Some(n) = model::move_to_location(data, index, &loc)
        {
            state.selected = Some(n);
            changed = true;
        }
    });
    if changed {
        return true;
    }
    let expected_wear_slot = item_wear_slot(&id, stats);
    if let Location::Worn(w) = &loc
        && !w.is_empty()
        && expected_wear_slot != Some(w.as_str())
        && model::group_items(data)
            .iter()
            .any(|g| g.location == loc && g.indices.first() == Some(&index))
    {
        ui.colored_label(
            colors::WARN,
            names
                .text("⚠ This item belongs in wear slot {expected}; it is assigned to {actual}.")
                .replace("{expected}", expected_wear_slot.unwrap_or("none"))
                .replace("{actual}", w),
        );
    }

    let Some(obj) = data
        .get_mut("items")
        .and_then(|a| a.get_mut(index))
        .and_then(Value::as_object_mut)
    else {
        return changed;
    };
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        changed |= item_fields(ui, obj, &id, names);
    });

    // Components
    ui.add_space(8.0);
    ui.label(RichText::new(names.text("Components").to_string()).strong());
    let comps = ensure_components(data, index);
    if let Some(comps) = comps {
        changed |= components_editor(ui, comps, def, names);
    }
    changed
}

fn item_fields(ui: &mut Ui, obj: &mut Map<String, Value>, id: &str, names: Names) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        row_label(ui, names, "Item id");
        let mut text = id.to_owned();
        let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(180.0));
        if r.changed() {
            obj.insert("id".into(), Value::String(text));
            changed = true;
        }
        if catalog::item(id).is_none() {
            ui.colored_label(colors::WARN, names.text("unknown id").to_string());
        }
    });
    if let Some(slot) = obj.get_mut("condition") {
        ui.horizontal(|ui| {
            row_label(ui, names, "Condition");
            let mut v = slot.as_f64().unwrap_or(1.0);
            let spec = crate::schema::FieldSpec {
                key: "condition",
                label: "Condition",
                help: "",
                kind: crate::schema::Kind::Gauge,
                min: 0.0,
                max: 1.0,
                unit: "",
                good: crate::schema::Good::High,
            };
            if gauge_bar(ui, &spec, &mut v, 200.0) {
                json::set_number_preserving(slot, v);
                changed = true;
            }
            changed |= number_editor(ui, slot, 0.005);
        });
    }
    if let Some(mut fav) = get_bool(obj, "favourited") {
        ui.horizontal(|ui| {
            row_label(ui, names, "Favourited");
            if ui.checkbox(&mut fav, "").changed() {
                json::set_bool(obj, "favourited", fav);
                changed = true;
            }
        });
    }
    if let Some(Value::String(label)) = obj.get_mut("label") {
        ui.horizontal(|ui| {
            row_label(ui, names, "Label");
            changed |= ui
                .add(
                    egui::TextEdit::singleline(label)
                        .hint_text(names.text("custom name").to_string())
                        .desired_width(180.0),
                )
                .changed();
        });
    }
    if let Some(slot) = obj.get_mut("swatch") {
        ui.horizontal(|ui| {
            row_label(ui, names, "Swatch");
            changed |= number_editor(ui, slot, 0.05);
        })
        .response
        .on_hover_text("Colour variant index");
    }
    let known = |k: &str| {
        matches!(
            k,
            "id" | "condition" | "slot" | "wearSlot" | "favourited" | "label" | "swatch"
        )
    };
    changed |= other_fields(ui, "item_other", obj, &known);
    changed
}

/// Returns the component dictionary of item `index`, creating it if needed.
fn ensure_components(data: &mut Value, index: usize) -> Option<&mut Map<String, Value>> {
    let root = data.as_object_mut()?;
    let len = root
        .get("items")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let comps = root.entry("itemComponents").or_insert_with(|| json!([]));
    let arr = comps.as_array_mut()?;
    if arr.len() < len {
        arr.resize(len, json!({}));
    }
    let slot = arr.get_mut(index)?;
    if !slot.is_object() {
        *slot = json!({});
    }
    slot.as_object_mut()
}

pub(super) const WATER: &str =
    "WaterContainerItem, Assembly-CSharp, Version=0.0.0.0, Culture=neutral, PublicKeyToken=null";

fn components_editor(
    ui: &mut Ui,
    comps: &mut Map<String, Value>,
    def: Option<&catalog::ItemDef>,
    names: Names,
) -> bool {
    let mut changed = false;
    if comps.is_empty() {
        ui.weak(
            names
                .text("No saved component state (the game uses the item's defaults).")
                .to_string(),
        );
    }
    let keys: Vec<String> = comps.keys().cloned().collect();
    let mut remove = None;
    for key in keys {
        let short = component_short_name(&key).to_owned();
        let Some(value) = comps.get_mut(&key) else {
            continue;
        };
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&short).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("🗑")
                        .on_hover_text(
                            names
                                .text("Remove this component (use defaults)")
                                .to_string(),
                        )
                        .clicked()
                    {
                        remove = Some(key.clone());
                    }
                });
            });
            let Some(obj) = value.as_object_mut() else {
                changed |= raw::subtree(ui, &key, &short, value);
                return;
            };
            changed |= match short.as_str() {
                "WaterContainerItem" => liquid_editor(ui, obj, def, names),
                "BatteryItem" => battery_editor(ui, obj, names),
                _ => generic_component(ui, &key, obj),
            };
        });
    }
    if let Some(k) = remove {
        comps.shift_remove(&k);
        changed = true;
    }
    if def.is_some_and(|d| d.capacity > 0.0)
        && !comps.contains_key(WATER)
        && ui
            .button(format!("💧 {}", names.text("Add liquid contents")))
            .clicked()
    {
        comps.insert(WATER.into(), json!({"stack": []}));
        changed = true;
    }
    changed
}

fn generic_component(ui: &mut Ui, key: &str, obj: &mut Map<String, Value>) -> bool {
    let mut changed = false;
    let keys: Vec<String> = obj.keys().cloned().collect();
    if keys.is_empty() {
        ui.weak("(empty)");
    }
    for k in keys {
        let Some(v) = obj.get_mut(&k) else { continue };
        changed |= if v.is_object() || v.is_array() {
            raw::subtree(ui, &format!("{key}/{k}"), &k, v)
        } else {
            generic_row(ui, &k, v)
        };
    }
    changed
}

fn liquid_editor(
    ui: &mut Ui,
    obj: &mut Map<String, Value>,
    def: Option<&catalog::ItemDef>,
    names: Names,
) -> bool {
    let mut changed = false;
    let capacity = def.map_or(0.0, |d| d.capacity as f64);
    let Some(Value::Array(stack)) = obj.get_mut("stack") else {
        return generic_component(ui, "liquid", obj);
    };
    let total: f64 = stack
        .iter()
        .filter_map(|s| s.get("amount").and_then(Value::as_f64))
        .sum();
    if capacity > 0.0 {
        let t = (total / capacity) as f32;
        ui.add(
            ProgressBar::new(t.min(1.0))
                .fill(if t > 1.0 {
                    colors::WARN
                } else {
                    colors::NEUTRAL
                })
                .text(format!("{} / {} ml", fmt_num(total), fmt_num(capacity))),
        );
    } else {
        ui.label(format!("Total: {} ml", fmt_num(total)));
    }
    let mut remove = None;
    for (i, entry) in stack.iter_mut().enumerate() {
        let Some(e) = entry.as_object_mut() else {
            continue;
        };
        ui.horizontal(|ui| {
            let mut liquid = get_str(e, "liquidId").unwrap_or("").to_owned();
            egui::ComboBox::from_id_salt(("liquid", i))
                .selected_text(names.liquid(&liquid))
                .width(180.0)
                .height(400.0)
                .show_ui(ui, |ui| {
                    for l in catalog::LIQUIDS {
                        ui.selectable_value(
                            &mut liquid,
                            (*l).to_owned(),
                            format!("{} ({l})", names.liquid(l)),
                        );
                    }
                });
            if Some(liquid.as_str()) != get_str(e, "liquidId") {
                e.insert("liquidId".into(), Value::String(liquid));
                changed = true;
            }
            if let Some(slot) = e.get_mut("amount") {
                let mut v = slot.as_f64().unwrap_or(0.0);
                if ui
                    .add(
                        egui::DragValue::new(&mut v)
                            .range(0.0..=f64::MAX)
                            .speed(1.0)
                            .suffix(" ml")
                            .max_decimals(2),
                    )
                    .changed()
                {
                    json::set_number_preserving(slot, v);
                    changed = true;
                }
            }
            if ui.small_button("🗑").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        stack.remove(i);
        changed = true;
    }
    ui.horizontal(|ui| {
        if ui
            .button(format!("➕ {}", names.text("Add liquid")))
            .clicked()
        {
            stack.push(json!({"liquidId": "water", "amount": 0.0}));
            changed = true;
        }
        if capacity > 0.0
            && total > 0.0
            && ui
                .button(names.text("Fill to capacity").to_string())
                .on_hover_text(names.text("Scales all liquids proportionally").to_string())
                .clicked()
        {
            let k = capacity / total;
            for e in stack.iter_mut().filter_map(Value::as_object_mut) {
                if let Some(a) = get_f64(e, "amount") {
                    json::set_f64(e, "amount", a * k);
                }
            }
            changed = true;
        }
        if !stack.is_empty() && ui.button(names.text("Empty").to_string()).clicked() {
            stack.clear();
            changed = true;
        }
    });
    changed
}

fn battery_editor(ui: &mut Ui, obj: &mut Map<String, Value>, names: Names) -> bool {
    let mut changed = false;
    if let Some(slot) = obj.get_mut("maxCharge") {
        ui.horizontal(|ui| {
            row_label(ui, names, "Max charge");
            changed |= number_editor(ui, slot, 1.0);
        });
    }
    if let Some(Value::String(kind)) = obj.get_mut("batteryType") {
        ui.horizontal(|ui| {
            row_label(ui, names, "Battery");
            let before = kind.clone();
            egui::ComboBox::from_id_salt("battery_type")
                .selected_text(if kind.is_empty() {
                    "(none)"
                } else {
                    kind.as_str()
                })
                .show_ui(ui, |ui| {
                    for (v, t) in [
                        ("", "(none)"),
                        ("smallbattery", "Small battery"),
                        ("mediumbattery", "Medium battery"),
                    ] {
                        ui.selectable_value(kind, v.to_owned(), t);
                    }
                });
            changed |= *kind != before;
        });
    }
    let known = |k: &str| matches!(k, "maxCharge" | "batteryType");
    let keys: Vec<String> = obj.keys().filter(|k| !known(k)).cloned().collect();
    for k in keys {
        if let Some(v) = obj.get_mut(&k) {
            changed |= generic_row(ui, &k, v);
        }
    }
    changed
}
