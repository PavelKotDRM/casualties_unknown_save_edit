//! Detailed item editor, including location, stats, warnings, and save fields.

#[path = "components.rs"]
mod components;
#[cfg(test)]
pub(super) use components::WATER;
use std::collections::HashMap;

use egui::{ProgressBar, RichText, Ui};
use serde_json::{Map, Value, json};

use super::{
    InventoryState, Names, card, colors, component_short_name, group_icon, holder_rules,
    is_hand_slot, item_wear_slot, items, location_combo, missing_wear_limb, row_label,
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
    header(ui, names, &id, def);
    stats_panel(ui, data, index, &id, names, stats);
    let mut loc = Location::of(&item);
    if placement_controls(ui, data, names, stats, state, index, &mut loc) {
        return true;
    }
    warnings(ui, data, &loc, &id, names, stats, index);
    if ammunition_controls(ui, data, names, state, index) {
        return true;
    }
    let mut edited_item = item.clone();
    let mut edited_components = match data.get("itemComponents") {
        None => json!({}),
        Some(Value::Array(components)) => {
            components.get(index).cloned().unwrap_or_else(|| json!({}))
        }
        Some(_) => {
            ui.colored_label(colors::BAD, names.text("Invalid item components"));
            return false;
        }
    };
    let metadata = crate::game_data::runtime::metadata();
    let original_components = edited_components.clone();
    let content_state = model::item_content_state(&item, &edited_components, metadata);
    if let Err(error) = &content_state {
        ui.colored_label(colors::BAD, names.text(error));
    }
    let derived = content_state
        .as_ref()
        .ok()
        .and_then(|state| state.condition);
    let read_only = derived.is_some() || def.is_some_and(|definition| definition.capacity > 0.0);
    let Some(obj) = edited_item.as_object_mut() else {
        ui.colored_label(colors::BAD, names.text("Invalid item object"));
        return false;
    };
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        changed |= item_fields(ui, obj, &id, names, derived, read_only);
    });

    ui.add_space(8.0);
    ui.label(RichText::new(names.text("Components").to_string()).strong());
    let mut synchronize = false;
    if let Ok(content) = &content_state {
        let condition_mismatch = content.condition.is_some_and(|expected| {
            item["condition"].as_f64().map(|condition| condition as f32) != Some(expected as f32)
        });
        let battery_mismatch = content.battery_capacity.is_some_and(|expected| {
            edited_components
                .get(crate::game_data::component_key("BatteryItem"))
                .is_some_and(|battery| battery["maxCharge"].as_f64() != Some(expected))
        });
        if condition_mismatch || battery_mismatch {
            ui.colored_label(
                colors::WARN,
                names.text("Saved state does not match the contents."),
            );
            synchronize = ui.button(names.text("Synchronize with contents")).clicked();
        }
    }
    if edited_item["id"] == item["id"] {
        let components_changed = if let Some(comps) = edited_components.as_object_mut() {
            components::components_editor(ui, comps, &mut edited_item, def, names)
        } else {
            raw::subtree(ui, "item_components", "Components", &mut edited_components)
        };
        if components_changed || synchronize || edited_item["condition"] != item["condition"] {
            match model::synchronize_item_state(&mut edited_item, &mut edited_components, metadata)
            {
                Ok(_) => changed = true,
                Err(error) => {
                    state.item_error = Some((index, error));
                    changed = false;
                }
            }
        }
    }
    if changed {
        if edited_components != original_components {
            match (
                edited_components.as_object(),
                ensure_components(data, index),
            ) {
                (Some(edited), Some(components)) => *components = edited.clone(),
                _ => {
                    state.item_error =
                        Some((index, names.text("Invalid item components").into_owned()));
                    changed = false;
                }
            }
        }
        if changed {
            data["items"][index] = edited_item;
            state.item_error = None;
        }
    }
    if let Some((failed_index, error)) = &state.item_error
        && *failed_index == index
    {
        ui.colored_label(colors::BAD, names.text(error));
    }
    changed
}

fn ammunition_controls(
    ui: &mut Ui,
    data: &mut Value,
    names: Names,
    state: &mut InventoryState,
    index: usize,
) -> bool {
    let status = match model::ammunition_status(data, index) {
        Ok(Some(status)) => status,
        Ok(None) => return false,
        Err(error) => {
            ui.colored_label(colors::WARN, names.text(&error).into_owned());
            return false;
        }
    };
    let mut changed = false;
    card(ui, |ui| {
        ui.strong(names.text("Ammunition"));
        let (rounds, capacity) = match status {
            model::AmmunitionStatus::Round => {
                ui.weak(names.text("Select a magazine or weapon to reload it."));
                return;
            }
            model::AmmunitionStatus::Magazine { rounds, capacity } => (rounds, capacity),
            model::AmmunitionStatus::Weapon {
                rounds,
                capacity,
                magazine,
                chamber,
            } => {
                let chamber = match chamber {
                    model::ChamberState::Round => "Live round",
                    model::ChamberState::Casing => "Spent casing",
                    model::ChamberState::Empty => "No round",
                };
                ui.label(format!(
                    "{}: {}",
                    names.text("Chamber"),
                    names.text(chamber)
                ));
                if !magazine {
                    ui.weak(names.text("No magazine inserted"));
                }
                (rounds, capacity)
            }
        };
        ui.add(
            ProgressBar::new(rounds as f32 / capacity as f32)
                .text(format!("{}: {rounds} / {capacity}", names.text("Rounds"))),
        );
        if ui.button(names.text("Reload from inventory"))
            .on_hover_text(names.text("Uses compatible non-favourite ammunition. Inserts a magazine when needed and chambers a round. Safety is unchanged."))
            .clicked()
        {
            match model::reload_ammunition(data, index) {
                Ok(index) => { state.selected = Some(index); changed = true; }
                Err(error) => { crate::ui::operation_result(ui, Err(error)); }
            }
        }
    });
    changed
}

fn header(ui: &mut Ui, names: Names, id: &str, def: Option<&catalog::ItemDef>) {
    ui.heading(format!(
        "{} {}",
        def.map_or("❓", |d| group_icon(d.group)),
        names.item(id)
    ));
    if let Some(d) = names.item_description(id) {
        ui.label(RichText::new(d).italics());
    }
    if let Some(d) = def {
        ui.weak(format!(
            "{} {} ({} {}){}{}",
            names.text("Category:"),
            names.text(catalog::group(d.group).label),
            names.text("Game category:"),
            names.stat(d.category),
            if d.wear_slot.is_empty() {
                String::new()
            } else {
                format!(
                    " · {} ({})",
                    names.text("wearable"),
                    names.wear_slot(d.wear_slot)
                )
            },
            if d.capacity > 0.0 {
                format!(" · holds {} ml", fmt_num(d.capacity as f64))
            } else {
                String::new()
            },
        ));
    }
}

fn stats_panel(
    ui: &mut Ui,
    data: &Value,
    index: usize,
    id: &str,
    names: Names,
    stats: &HashMap<String, ItemStats>,
) {
    let weights = model::item_weights(data, index, stats);
    if let Some(item_stats) = stats.get(id) {
        if let Some(game_weight) = weights.game {
            ui.weak(format!(
                "{} {:.2} u{}{}",
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
        if let (Some(capacity), Some(max_item_weight), Some(multiplier)) = (
            item_stats.container_capacity,
            item_stats.container_max_item_weight,
            item_stats.container_encumbrance,
        ) {
            let space = model::container_space(data, index, stats);
            let (space_text, space_color) = match space {
                Some(space) if space.complete => {
                    let mut text = format!(
                        "{} {:.2} / {:.2} u",
                        names.text("Capacity remaining:"),
                        space.remaining,
                        space.capacity,
                    );
                    if space.over_capacity > 0.0 {
                        let over = format!("{:.2}", space.over_capacity);
                        text.push_str(" · ");
                        text.push_str(
                            &names
                                .text("Over capacity by {weight} u")
                                .replace("{weight}", &over),
                        );
                    }
                    (
                        text,
                        if space.over_capacity > 0.0 {
                            colors::BAD
                        } else {
                            colors::NEUTRAL
                        },
                    )
                }
                _ => (
                    format!(
                        "{} — / {:.2} u",
                        names.text("Capacity remaining:"),
                        capacity,
                    ),
                    colors::WARN,
                ),
            };
            ui.colored_label(space_color, space_text).on_hover_text(names.text("Remaining capacity is maximum weight minus the full weight of stored items; encumbrance reduction affects carried load, not storage capacity.").to_string());
            ui.weak(format!(
                "{} {:.2} u · {} {:.0}%",
                names.text("Max weight/item:"),
                max_item_weight,
                names.text("Encumbrance reduction:"),
                (1.0 - multiplier) * 100.0,
            ));
        }
        if item_stats.liquid_capacity.is_some() {
            if let (Some(liquid), Some(physical)) = (weights.liquid, weights.physical) {
                ui.weak(format!(
                    "{} ≈{:.2} kg · {} ≈{liquid:.2} kg · {} ≈{physical:.2} kg",
                    names.text("Empty container:"),
                    item_stats.total_weight(0.0),
                    names.text("Liquid:"),
                    names.text("Physical weight:"),
                )).on_hover_text(names.text("Approximation: liquids use 1 kg per litre; game weight units are treated as kg for empty item weight. Container encumbrance modifiers do not apply.").to_string());
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
}

fn placement_controls(
    ui: &mut Ui,
    data: &mut Value,
    names: Names,
    stats: &HashMap<String, ItemStats>,
    state: &mut InventoryState,
    index: usize,
    loc: &mut Location,
) -> bool {
    let mut changed = false;
    let holder = holder_rules(
        items(data)
            .get(index)
            .and_then(|item| item["id"].as_str())
            .unwrap_or(""),
        stats,
    );
    // Location (moving changes the index).
    ui.horizontal(|ui| {
        row_label(ui, names, "Location");
        if location_combo(
            ui,
            "item_location",
            data,
            loc,
            holder,
            true,
            names,
            stats,
            index,
        ) && let Some(n) = model::move_to_location(data, index, loc)
        {
            state.selected = Some(n);
            changed = true;
        }
    });
    if changed {
        return true;
    }
    changed
}

fn warnings(
    ui: &mut Ui,
    data: &Value,
    loc: &Location,
    id: &str,
    names: Names,
    stats: &HashMap<String, ItemStats>,
    index: usize,
) {
    let expected_wear_slot = item_wear_slot(id, stats);
    let is_holder = model::group_items(data)
        .iter()
        .any(|g| g.location == *loc && g.indices.first() == Some(&index));
    let holder = holder_rules(id, stats);
    if let Location::Slot(_) = loc
        && is_holder
        && holder.not_holdable
    {
        ui.colored_label(
            colors::WARN,
            names.text(
                "⚠ The game doesn't let clothing be held; wear it or store it in a container.",
            ),
        );
    } else if let Location::Slot(slot) = loc
        && is_holder
        && holder.hands_only
        && !is_hand_slot(*slot)
    {
        ui.colored_label(
            colors::WARN,
            names.text("⚠ The game only lets this item be held in hands; move it to a hand slot."),
        );
    }
    if let Location::Worn(w) = loc
        && !w.is_empty()
        && expected_wear_slot != Some(w.as_str())
        && model::group_items(data)
            .iter()
            .any(|g| g.location == *loc && g.indices.first() == Some(&index))
    {
        ui.colored_label(
            colors::WARN,
            names
                .text("⚠ This item belongs in wear slot {expected}; it is assigned to {actual}.")
                .replace(
                    "{expected}",
                    &expected_wear_slot.map_or_else(
                        || names.text("None").into_owned(),
                        |slot| names.wear_slot(slot),
                    ),
                )
                .replace("{actual}", &names.wear_slot(w)),
        );
    }
    if let Location::Worn(w) = loc
        && is_holder
        && expected_wear_slot == Some(w.as_str())
        && let Some(limb) = missing_wear_limb(data, holder)
    {
        ui.colored_label(
            colors::BAD,
            names
                .text(
                    "⚠ {limb} is dismembered: the game won't wear this item and drops it on load.",
                )
                .replace("{limb}", &names.limb(limb)),
        );
    }
    if let Location::Slot(slot) = loc
        && usize::try_from(*slot).map_or(true, |s| s >= crate::schema::HOLD_SLOTS.len())
    {
        ui.colored_label(
            colors::BAD,
            names.text(
                "⚠ The game has no such slot: loading fails for this item and shifts the saved component data of later items.",
            ),
        );
    }
    let condition = items(data)
        .get(index)
        .and_then(|item| item.get("condition"))
        .and_then(Value::as_f64);
    if condition.is_some_and(|c| c <= 0.0)
        && stats.get(id).is_some_and(|s| s.destroy_at_zero_condition)
    {
        ui.colored_label(
            colors::BAD,
            names.text(
                "⚠ Condition 0: the game destroys this item (a container spills its contents).",
            ),
        );
    }
}

fn item_fields(
    ui: &mut Ui,
    obj: &mut Map<String, Value>,
    id: &str,
    names: Names,
    derived: Option<f64>,
    read_only: bool,
) -> bool {
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
            let mut v = derived.or_else(|| slot.as_f64()).unwrap_or(1.0);
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
            ui.add_enabled_ui(!read_only, |ui| {
                if gauge_bar(ui, &spec, &mut v, 200.0) {
                    json::set_number_preserving(slot, v);
                    changed = true;
                }
                if read_only {
                    number_editor(ui, &mut json!(v), 0.005);
                } else {
                    changed |= number_editor(ui, slot, 0.005);
                }
            })
            .response
            .on_disabled_hover_text(
                names.text("Condition is calculated from contents. Edit the contents below."),
            );
        });
    }
    if !read_only && let Some(rules) = crate::game_data::runtime::metadata().condition_uses.get(id)
    {
        for rule in rules {
            let mut item = Value::Object(obj.clone());
            match uses_editor(ui, &mut item, *rule, names) {
                Ok(true) => {
                    *obj = item.as_object().expect("item object").clone();
                    changed = true;
                }
                Ok(false) => {}
                Err(error) => {
                    ui.colored_label(colors::BAD, names.text(&error));
                }
            }
        }
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
            changed |= components::swatch_picker(ui, slot);
        })
        .response
        .on_hover_text(names.text("Colour variant index"));
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

fn uses_editor(
    ui: &mut Ui,
    item: &mut Value,
    rule: crate::game_data::effects::ConditionUse,
    names: Names,
) -> Result<bool, String> {
    let condition = item["condition"].as_f64().ok_or("Invalid item condition")?;
    let mut count = model::remaining_uses(condition as f32, rule)?;
    let full = model::remaining_uses(1.0, rule)?;
    let maximum = full.max(count);
    ui.horizontal(|ui| {
        row_label(
            ui,
            names,
            if rule.on_limb { "Uses remaining (limb)" } else { "Uses remaining" },
        );
        if ui
            .add(egui::DragValue::new(&mut count).range(0..=maximum))
            .on_hover_text(names.text("Calculated from the game's fixed condition cost and threshold. Other action requirements still apply."))
            .changed()
        {
            model::set_remaining_uses(item, rule, count)
        } else {
            Ok(false)
        }
    })
    .inner
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
