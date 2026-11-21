//! Editors for item-specific component state and liquid containers.

use super::*;

/// `Item.ColorSwatches` from the game; `swatch` indexes this table.
const SWATCH_COLORS: [[f32; 3]; 8] = [
    [1.0, 1.0, 1.0],
    [0.5, 0.5, 0.5],
    [0.28, 1.0, 0.28],
    [0.96, 0.67, 0.17],
    [0.9, 0.333, 1.0],
    [0.4, 0.333, 1.0],
    [0.2, 1.0, 1.0],
    [1.0, 0.988, 0.4],
];
pub(super) fn swatch_picker(ui: &mut Ui, value: &mut Value) -> bool {
    let current = value.as_i64();
    let mut changed = false;
    for (index, [r, g, b]) in SWATCH_COLORS.iter().enumerate() {
        let color =
            egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
        let selected = current == Some(index as i64);
        let text = RichText::new(if selected { "■" } else { "□" })
            .color(color)
            .size(18.0);
        if ui.selectable_label(selected, text).clicked() && !selected {
            *value = json!(index);
            changed = true;
        }
    }
    if !current.is_some_and(|c| (0..SWATCH_COLORS.len() as i64).contains(&c)) {
        ui.colored_label(colors::BAD, format!("⚠ {value}"));
    }
    changed
}
pub(in crate::ui::inventory) const WATER: &str =
    "WaterContainerItem, Assembly-CSharp, Version=0.0.0.0, Culture=neutral, PublicKeyToken=null";
pub(super) fn components_editor(
    ui: &mut Ui,
    comps: &mut Map<String, Value>,
    item: &mut Value,
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
                "BatteryItem" => battery_editor(ui, obj, item, names),
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

fn battery_editor(
    ui: &mut Ui,
    obj: &mut Map<String, Value>,
    item: &mut Value,
    names: Names,
) -> bool {
    let mut changed = false;
    let metadata = crate::game_data::runtime::metadata();
    let id = item["id"].as_str().unwrap_or("").to_owned();
    if metadata
        .battery_limits
        .get(&id)
        .is_none_or(|limit| !limit.is_finite() || *limit <= 0.0)
    {
        ui.colored_label(
            colors::BAD,
            names
                .text("Battery compatibility for {id} is unavailable")
                .replace("{id}", &id),
        );
    }
    if let Some(Value::String(kind)) = obj.get_mut("batteryType") {
        ui.horizontal(|ui| {
            row_label(ui, names, "Battery");
            let before = kind.clone();
            egui::ComboBox::from_id_salt("battery_type")
                .selected_text(if kind.is_empty() {
                    names.text("No battery").into_owned()
                } else {
                    names.item(kind)
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(kind, String::new(), names.text("No battery"));
                    for battery in metadata.battery_charges.keys() {
                        if let Ok(capacity) = model::battery_capacity(&id, battery, metadata) {
                            ui.selectable_value(
                                kind,
                                battery.clone(),
                                format!("{} ({})", names.item(battery), fmt_num(capacity)),
                            );
                        }
                    }
                });
            changed |= *kind != before;
        });
        if changed && kind.is_empty() {
            obj.insert("batteryWasFavourited".into(), json!(false));
        }
    } else {
        ui.colored_label(colors::BAD, names.text("Invalid battery type"));
        changed |= generic_component(ui, "battery", obj);
        return changed;
    }
    match model::battery_capacity(&id, get_str(obj, "batteryType").unwrap_or(""), metadata) {
        Ok(capacity) => {
            ui.horizontal(|ui| {
                row_label(ui, names, "Max charge");
                ui.label(fmt_num(capacity));
            });
            if capacity > 0.0 {
                if let Some(condition) = item["condition"].as_f64() {
                    ui.horizontal(|ui| {
                        row_label(ui, names, "Charge");
                        let mut charge = condition * capacity;
                        if ui
                            .add(
                                egui::DragValue::new(&mut charge)
                                    .range(0.0..=capacity)
                                    .speed(0.5),
                            )
                            .changed()
                        {
                            match model::set_battery_charge(item, charge, capacity) {
                                Ok(updated) => changed |= updated,
                                Err(error) => {
                                    ui.colored_label(colors::BAD, names.text(&error));
                                }
                            }
                        }
                    });
                } else {
                    ui.colored_label(colors::BAD, names.text("Invalid item condition"));
                }
            }
        }
        Err(error) => {
            ui.colored_label(colors::BAD, names.text(&error));
        }
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
