//! Inventory tab: items grouped by location (held slot / worn slot) with the
//! container contents, an item details editor and an "add item" catalogue.

use std::collections::HashMap;

use egui::{ProgressBar, RichText, Ui};
use serde_json::{Map, Value, json};

use super::{Names, card, category_icon, colors, quality_color};
use crate::catalog;
use crate::game_data::ItemStats;
use crate::json::{self, get_bool, get_f64, get_str};
use crate::model::{self, Location};

/// UI state of the inventory tab.
#[derive(Default)]
pub struct InventoryState {
    /// Selected item index.
    pub selected: Option<usize>,
    /// Whether the "add item" window is open.
    pub add_open: bool,
    /// Search text of the catalogue.
    pub add_search: String,
    /// Category filter of the catalogue (empty = all).
    pub add_category: String,
    /// Selected catalogue item id.
    pub add_id: String,
    /// Target location of new items.
    pub add_location: Option<Location>,
}

/// Action requested by an item row.
enum RowAction {
    Select(usize),
    Up(usize),
    Down(usize),
    Duplicate(usize),
    Delete(usize),
    ToggleFavourite(usize),
}

/// Human-readable name of a location.
pub fn location_name(loc: &Location, language: super::InterfaceLanguage) -> String {
    match loc {
        Location::Slot(s) => format!("{} {s}", language.text("Inventory slot")),
        Location::Worn(w) if w.is_empty() => language
            .text("Unassigned (slot -1, no wear slot)")
            .into_owned(),
        Location::Worn(w) => format!("{}: {w}", language.text("Worn")),
    }
}

fn location_icon(loc: &Location) -> &'static str {
    match loc {
        Location::Slot(_) => "✋",
        Location::Worn(_) => "👕",
    }
}

/// Short name of a component type (`WaterContainerItem, Assembly-CSharp…`).
pub fn component_short_name(full: &str) -> &str {
    full.split(',').next().unwrap_or(full).trim()
}

fn item_wear_slot<'a>(id: &str, stats: &'a HashMap<String, ItemStats>) -> Option<&'a str> {
    if let Some(item) = stats.get(id) {
        return item.wearable.then_some(item.wear_slot.as_deref()).flatten();
    }
    catalog::item(id)
        .filter(|item| !item.wear_slot.is_empty())
        .map(|item| item.wear_slot)
}

/// Locations offered in choosers, with only compatible wear slots.
fn location_choices(
    data: &Value,
    allowed_wear_slot: Option<&str>,
    current: &Location,
    preserve_current: bool,
) -> Vec<Location> {
    let mut out: Vec<Location> = model::group_items(data)
        .into_iter()
        .map(|g| g.location)
        .filter(|location| match location {
            Location::Slot(_) => true,
            Location::Worn(slot) => allowed_wear_slot == Some(slot.as_str()),
        })
        .collect();
    for s in 0..10 {
        let l = Location::Slot(s);
        if !out.contains(&l) {
            out.push(l);
        }
    }
    if let Some(slot) = allowed_wear_slot {
        let l = Location::Worn(slot.to_owned());
        if !out.contains(&l) {
            out.push(l);
        }
    }
    if preserve_current && !out.contains(current) {
        out.push(current.clone());
    }
    out
}

fn location_combo(
    ui: &mut Ui,
    id: &str,
    data: &Value,
    current: &mut Location,
    allowed_wear_slot: Option<&str>,
    preserve_current: bool,
    language: super::InterfaceLanguage,
) -> bool {
    let before = current.clone();
    egui::ComboBox::from_id_salt(id)
        .selected_text(location_name(current, language))
        .width(240.0)
        .height(400.0)
        .show_ui(ui, |ui| {
            for loc in location_choices(data, allowed_wear_slot, current, preserve_current) {
                let text = format!("{} {}", location_icon(&loc), location_name(&loc, language));
                ui.selectable_value(current, loc, text);
            }
        });
    *current != before
}

fn items(data: &Value) -> &[Value] {
    data.get("items")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Shows the inventory tab. Returns `true` if the document changed.
pub fn show(
    ui: &mut Ui,
    data: &mut Value,
    names: Names,
    stats: &HashMap<String, ItemStats>,
    state: &mut InventoryState,
) -> bool {
    let mut changed = false;
    let count = items(data).len();
    if state.selected.is_some_and(|i| i >= count) {
        state.selected = None;
    }
    ui.horizontal(|ui| {
        if ui
            .button(format!("➕ {}", names.text("Add item…")))
            .clicked()
        {
            state.add_open = true;
            if state.add_location.is_none() {
                state.add_location = Some(
                    state
                        .selected
                        .and_then(|i| items(data).get(i))
                        .map_or(Location::Slot(0), Location::of),
                );
            }
        }
        ui.separator();
        ui.label(format!("{count} {}", names.text("items")));
        ui.weak(format!(
            "· {}",
            names
                .text("The first item of a location is held / worn; the following ones are stored inside it.")
        ));
    });
    ui.horizontal_wrapped(|ui| {
        if let Some(load) = model::carry_load(data, stats) {
            let color = if load.complete && load.weight > load.capacity {
                colors::BAD
            } else {
                colors::NEUTRAL
            };
            ui.colored_label(
                color,
                format!(
                    "{}: {:.1} / {:.1} kg",
                    names.text("Game load"),
                    load.weight,
                    load.capacity
                ),
            ).on_hover_text(names.text("Game load uses item weights and container encumbrance multipliers. Liquid containers scale with their fill level.").to_string());
            if !load.complete {
                ui.colored_label(
                    colors::WARN,
                    names.text("Incomplete: item, container or liquid data is unavailable.")
                        .to_string(),
                );
            } else if load.weight > load.capacity {
                ui.colored_label(
                    colors::BAD,
                    names.text("⚠ Character is overburdened.").to_string(),
                );
            }
        } else {
            ui.colored_label(
                colors::WARN,
                names
                    .text("Carry capacity cannot be calculated from this save.")
                    .to_string(),
            );
        }
        let physical = model::physical_load(data, stats);
        ui.separator();
        ui.label(format!("{}: ≈{:.2} kg", names.text("Physical weight"), physical.weight))
            .on_hover_text(names.text("Approximation: liquids use 1 kg per litre; empty container weight is inferred from the game's minimum item weight. Container encumbrance modifiers do not apply.").to_string());
        if !physical.complete {
            ui.colored_label(colors::WARN, names.text("Physical estimate incomplete").to_string());
        }
    });
    ui.separator();

    let mut action = None;
    ui.horizontal_top(|ui| {
        let list_width = (ui.available_width() * 0.5).max(380.0);
        ui.vertical(|ui| {
            ui.set_width(list_width);
            egui::ScrollArea::vertical()
                .id_salt("inv_list")
                .auto_shrink(false)
                .show(ui, |ui| {
                    action = item_list(ui, data, names, stats, state.selected);
                });
        });
        ui.separator();
        ui.vertical(|ui| {
            egui::ScrollArea::vertical()
                .id_salt("inv_details")
                .auto_shrink(false)
                .show(ui, |ui| match state.selected {
                    Some(i) => changed |= details(ui, data, names, stats, state, i),
                    None => {
                        ui.add_space(20.0);
                        ui.weak(names.text("Select an item to edit it.").to_string());
                    }
                });
        });
    });

    if let Some(a) = action {
        changed |= apply_action(data, state, a);
    }
    if state.add_open {
        changed |= add_window(ui, data, names, stats, state);
    }
    changed
}

fn apply_action(data: &mut Value, state: &mut InventoryState, action: RowAction) -> bool {
    match action {
        RowAction::Select(i) => {
            state.selected = Some(i);
            false
        }
        RowAction::Up(i) | RowAction::Down(i) => {
            let up = matches!(action, RowAction::Up(_));
            if let Some(n) = model::move_within_group(data, i, up) {
                state.selected = Some(n);
                true
            } else {
                false
            }
        }
        RowAction::Duplicate(i) => {
            state.selected = model::duplicate_item(data, i).or(state.selected);
            true
        }
        RowAction::Delete(i) => {
            model::remove_item(data, i);
            state.selected = None;
            true
        }
        RowAction::ToggleFavourite(i) => {
            if let Some(obj) = data
                .get_mut("items")
                .and_then(|a| a.get_mut(i))
                .and_then(Value::as_object_mut)
            {
                let f = get_bool(obj, "favourited").unwrap_or(false);
                json::set_bool(obj, "favourited", !f);
            }
            true
        }
    }
}

fn item_list(
    ui: &mut Ui,
    data: &Value,
    names: Names,
    stats: &HashMap<String, ItemStats>,
    selected: Option<usize>,
) -> Option<RowAction> {
    let mut action = None;
    let all = items(data);
    for group in model::group_items(data) {
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(format!(
                    "{}  {}",
                    location_icon(&group.location),
                    location_name(&group.location, names.interface_language)
                ))
                .strong()
                .color(colors::ACCENT),
            );
            for (pos, &i) in group.indices.iter().enumerate() {
                let first = pos == 0;
                let last = pos + 1 == group.indices.len();
                ui.horizontal(|ui| {
                    if !first {
                        ui.add_space(22.0);
                        ui.weak("↳");
                    }
                    if let Some(a) = item_row(
                        ui,
                        &all[i],
                        i,
                        model::item_weights(data, i, stats),
                        names,
                        selected == Some(i),
                        first,
                        last,
                    ) {
                        action = Some(a);
                    }
                });
                if first && group.indices.len() > 1 {
                    ui.horizontal(|ui| {
                        ui.add_space(22.0);
                        ui.weak(format!(
                            "{} {} {}",
                            names.text("contains"),
                            group.indices.len() - 1,
                            names.text("item(s):")
                        ));
                    });
                }
            }
        });
        ui.add_space(4.0);
    }
    if all.is_empty() {
        ui.weak(names.text("The inventory is empty.").to_string());
    }
    action
}

fn item_row(
    ui: &mut Ui,
    item: &Value,
    i: usize,
    weights: model::ItemWeights,
    names: Names,
    selected: bool,
    first: bool,
    last: bool,
) -> Option<RowAction> {
    let mut action = None;
    let id = item.get("id").and_then(Value::as_str).unwrap_or("");
    let def = catalog::item(id);
    let icon = def.map_or("❓", |d| category_icon(d.category));
    let mut text = RichText::new(format!("{icon} {}", names.item(id)));
    if def.is_none() {
        text = text.color(colors::WARN);
    }
    let r = ui.selectable_label(selected, text);
    if r.clicked() {
        action = Some(RowAction::Select(i));
    }
    if let Some(weight) = weights.game {
        ui.weak(format!("{weight:.2} kg"))
            .on_hover_text(names.text("Game item weight").to_string());
    }
    r.on_hover_ui(|ui| {
        ui.strong(names.item(id));
        ui.weak(format!("id: {id} · items[{i}]"));
        if let Some(d) = names.item_description(id) {
            ui.label(d);
        }
        if def.is_none() {
            ui.colored_label(
                colors::WARN,
                names
                    .text("Unknown id — not in the game's item list.")
                    .to_string(),
            );
        }
    });
    // Align the condition bars in one column, regardless of indent and name length.
    let bar_x = ui.max_rect().left() + 250.0;
    let pad = bar_x - ui.cursor().left();
    if pad > 0.0 {
        ui.add_space(pad);
    }
    let cond = item.get("condition").and_then(Value::as_f64).unwrap_or(1.0) as f32;
    ui.add(
        ProgressBar::new(cond)
            .desired_width(60.0)
            .desired_height(10.0)
            .fill(quality_color(cond))
            .text(RichText::new(format!("{:.0}%", cond * 100.0)).small()),
    )
    .on_hover_text(names.text("Condition").to_string());
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui
            .small_button("🗑")
            .on_hover_text(names.text("Delete").to_string())
            .clicked()
        {
            action = Some(RowAction::Delete(i));
        }
        if ui
            .small_button("⧉")
            .on_hover_text(names.text("Duplicate").to_string())
            .clicked()
        {
            action = Some(RowAction::Duplicate(i));
        }
        if ui
            .add_enabled(!last, egui::Button::new("⏷").small())
            .on_hover_text(names.text("Move down").to_string())
            .clicked()
        {
            action = Some(RowAction::Down(i));
        }
        if ui
            .add_enabled(!first, egui::Button::new("⏶").small())
            .on_hover_text(names.text("Move up").to_string())
            .clicked()
        {
            action = Some(RowAction::Up(i));
        }
        let fav = item
            .get("favourited")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let star = if fav {
            RichText::new("★").color(colors::ACCENT)
        } else {
            RichText::new("☆")
        };
        if ui
            .small_button(star)
            .on_hover_text(names.text("Favourite").to_string())
            .clicked()
        {
            action = Some(RowAction::ToggleFavourite(i));
        }
    });
    action
}

fn row_label(ui: &mut Ui, names: Names, text: &str) {
    ui.add_sized(
        [120.0, ui.spacing().interact_size.y],
        egui::Label::new(names.text(text).to_string()),
    );
}

fn details(
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
                format!(" · holds {} ml", super::fmt_num(d.capacity as f64))
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
            format!(
                "{}",
                names
                    .text(
                        "⚠ This item belongs in wear slot {expected}; it is assigned to {actual}."
                    )
                    .replace("{expected}", expected_wear_slot.unwrap_or("none"))
                    .replace("{actual}", w)
            ),
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
            if super::gauge_bar(ui, &spec, &mut v, 200.0) {
                json::set_number_preserving(slot, v);
                changed = true;
            }
            changed |= super::number_editor(ui, slot, 0.005);
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
            changed |= super::number_editor(ui, slot, 0.05);
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
    changed |= super::other_fields(ui, "item_other", obj, &known);
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

const WATER: &str =
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
                changed |= super::raw::subtree(ui, &key, &short, value);
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
            super::raw::subtree(ui, &format!("{key}/{k}"), &k, v)
        } else {
            super::generic_row(ui, &k, v)
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
                .text(format!(
                    "{} / {} ml",
                    super::fmt_num(total),
                    super::fmt_num(capacity)
                )),
        );
    } else {
        ui.label(format!("Total: {} ml", super::fmt_num(total)));
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
            changed |= super::number_editor(ui, slot, 1.0);
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
            changed |= super::generic_row(ui, &k, v);
        }
    }
    changed
}

fn add_window(
    ui: &mut Ui,
    data: &mut Value,
    names: Names,
    stats: &HashMap<String, ItemStats>,
    state: &mut InventoryState,
) -> bool {
    let mut changed = false;
    let mut open = state.add_open;
    let mut close = false;
    let selected_id = state.add_id.clone();
    let allowed_wear_slot = item_wear_slot(&selected_id, stats);
    if matches!(
        state.add_location.as_ref(),
        Some(Location::Worn(slot)) if allowed_wear_slot != Some(slot.as_str())
    ) {
        state.add_location = Some(Location::Slot(0));
    }
    egui::Window::new(format!("➕ {}", names.text("Add item")))
        .open(&mut open)
        .default_size([520.0, 560.0])
        .collapsible(false)
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.add(
                    egui::TextEdit::singleline(&mut state.add_search)
                        .hint_text(names.text("Search name or id").to_string())
                        .desired_width(200.0),
                );
                let mut cats: Vec<&str> = catalog::ITEMS.iter().map(|d| d.category).collect();
                cats.sort_unstable();
                cats.dedup();
                egui::ComboBox::from_id_salt("add_cat")
                    .selected_text(if state.add_category.is_empty() {
                        names.text("All categories").to_string()
                    } else {
                        state.add_category.clone()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut state.add_category,
                            String::new(),
                            names.text("All categories").to_string(),
                        );
                        for c in cats {
                            ui.selectable_value(
                                &mut state.add_category,
                                c.to_owned(),
                                format!("{} {c}", category_icon(c)),
                            );
                        }
                    });
            });
            ui.separator();
            let needle = state.add_search.to_lowercase();
            let mut list: Vec<(&catalog::ItemDef, String)> = catalog::ITEMS
                .iter()
                .filter(|d| state.add_category.is_empty() || d.category == state.add_category)
                .map(|d| (d, names.item(d.id)))
                .filter(|(d, n)| {
                    needle.is_empty()
                        || d.id.contains(&needle)
                        || n.to_lowercase().contains(&needle)
                })
                .collect();
            list.sort_by_key(|a| a.1.to_lowercase());
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (d, name) in &list {
                        let r = ui
                            .selectable_label(
                                state.add_id == d.id,
                                format!("{} {name}   ", category_icon(d.category)),
                            )
                            .on_hover_text(format!("id: {}\ncategory: {}", d.id, d.category));
                        if r.clicked() {
                            state.add_id = d.id.to_owned();
                        }
                        if r.double_clicked() {
                            state.add_id = d.id.to_owned();
                            changed |= do_add(data, state, stats);
                        }
                    }
                    if list.is_empty() {
                        ui.weak(names.text("Nothing found.").to_string());
                    }
                });
            ui.separator();
            ui.horizontal(|ui| {
                row_label(ui, names, "Item id");
                ui.add(egui::TextEdit::singleline(&mut state.add_id).desired_width(180.0));
                if !state.add_id.is_empty() {
                    ui.label(names.item(&state.add_id));
                }
            });
            ui.horizontal(|ui| {
                row_label(ui, names, "Put into");
                let mut loc = state.add_location.clone().unwrap_or(Location::Slot(0));
                if matches!(
                    &loc,
                    Location::Worn(slot)
                        if item_wear_slot(&state.add_id, stats) != Some(slot.as_str())
                ) {
                    loc = Location::Slot(0);
                }
                location_combo(
                    ui,
                    "add_location",
                    data,
                    &mut loc,
                    item_wear_slot(&state.add_id, stats),
                    false,
                    names.interface_language,
                );
                state.add_location = Some(loc);
                if let Some(slot) = item_wear_slot(&state.add_id, stats)
                    && ui.small_button(format!("Wear ({slot})")).clicked()
                {
                    state.add_location = Some(Location::Worn(slot.to_owned()));
                }
            });
            if let Some(loc) = &state.add_location {
                let occupied = model::group_items(data).iter().any(|g| &g.location == loc);
                ui.weak(if occupied {
                    names.text("The location is occupied: the item will be stored inside the held / worn item.").to_string()
                } else {
                    names.text("The location is empty: the item will be held / worn.").to_string()
                });
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !state.add_id.trim().is_empty(),
                        egui::Button::new(format!("➕ {}", names.text("Add"))),
                    )
                    .clicked()
                {
                    changed |= do_add(data, state, stats);
                }
                if ui.button(names.text("Close").to_string()).clicked() {
                    close = true;
                }
            });
        });
    state.add_open = open && !close;
    changed
}

fn do_add(
    data: &mut Value,
    state: &mut InventoryState,
    stats: &HashMap<String, ItemStats>,
) -> bool {
    let mut loc = state.add_location.clone().unwrap_or(Location::Slot(0));
    if matches!(
        &loc,
        Location::Worn(slot) if item_wear_slot(state.add_id.trim(), stats) != Some(slot.as_str())
    ) {
        loc = Location::Slot(0);
    }
    let item = model::new_item(state.add_id.trim(), &loc);
    match model::add_item(data, item, json!({})) {
        Some(i) => {
            state.selected = Some(i);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_names_are_shortened() {
        assert_eq!(component_short_name(WATER), "WaterContainerItem");
        assert_eq!(component_short_name("Foo"), "Foo");
    }

    #[test]
    fn location_choices_only_offer_the_supported_wear_slot() {
        let data = json!({
            "items": [
                {"id": "coat", "slot": -1, "wearSlot": "torso"}
            ]
        });
        let choices = location_choices(&data, Some("head"), &Location::Slot(0), false);
        assert!(choices.contains(&Location::Worn("head".to_owned())));
        assert!(!choices.contains(&Location::Worn("torso".to_owned())));
    }

    #[test]
    fn existing_incompatible_wear_slot_remains_visible_for_repair() {
        let data = json!({
            "items": [
                {"id": "coat", "slot": -1, "wearSlot": "torso"}
            ]
        });
        let current = Location::Worn("torso".to_owned());
        let choices = location_choices(&data, Some("head"), &current, true);
        assert!(choices.contains(&current));
        assert!(choices.contains(&Location::Worn("head".to_owned())));
    }

    #[test]
    fn synced_nonwearable_stats_override_catalogue_wear_slots() {
        let item = catalog::ITEMS
            .iter()
            .find(|item| !item.wear_slot.is_empty())
            .unwrap();
        let stats = HashMap::from([(
            item.id.to_owned(),
            ItemStats {
                category: item.category.to_owned(),
                weight: 0.0,
                scale_weight_with_condition: false,
                wearable: false,
                wear_slot: None,
                armor: None,
                insulation: None,
                container_encumbrance: None,
                liquid_capacity: None,
            },
        )]);
        assert_eq!(item_wear_slot(item.id, &stats), None);
    }

    #[test]
    fn adding_an_item_does_not_keep_an_incompatible_wear_slot() {
        let mut data = json!({"items": []});
        let mut state = InventoryState {
            add_id: "cap".to_owned(),
            add_location: Some(Location::Worn("torso".to_owned())),
            ..Default::default()
        };
        let stats = HashMap::from([(
            "cap".to_owned(),
            ItemStats {
                category: "wearable".to_owned(),
                weight: 1.0,
                scale_weight_with_condition: false,
                wearable: true,
                wear_slot: Some("head".to_owned()),
                armor: None,
                insulation: None,
                container_encumbrance: None,
                liquid_capacity: None,
            },
        )]);

        assert!(do_add(&mut data, &mut state, &stats));
        assert_eq!(data["items"][0]["slot"], 0);
        assert_eq!(data["items"][0]["wearSlot"], Value::Null);
    }
}
