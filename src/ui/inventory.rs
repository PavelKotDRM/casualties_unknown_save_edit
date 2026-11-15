//! Inventory tab: items grouped by location (held slot / worn slot) with the
//! container contents, an item details editor and an "add item" catalogue.

mod add;
mod details;

use std::collections::HashMap;

use egui::{ProgressBar, RichText, Ui};
use serde_json::Value;

use super::{Names, card, category_icon, colors, quality_color};
use crate::catalog;
use crate::game_data::ItemStats;
use crate::json::{self, get_bool};
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

struct ItemRowState {
    index: usize,
    weights: model::ItemWeights,
    selected: bool,
    first: bool,
    last: bool,
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
                    "{}: {:.1} / {:.1} u",
                    names.text("Game load"),
                    load.weight,
                    load.capacity
                ),
            ).on_hover_text(names.text("Game load is shown in game units (u). Container capacity and per-item limits determine what can be stored; the encumbrance multiplier reduces the contribution of stored items. Liquid containers scale with their fill level.").to_string());
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
            .on_hover_text(names.text("Approximation: liquids use 1 kg per litre; game weight units are treated as kg for empty item weight. Container encumbrance modifiers do not apply.").to_string());
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
                    Some(i) => changed |= details::details(ui, data, names, stats, state, i),
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
        changed |= add::add_window(ui, data, names, stats, state);
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
                        names,
                        ItemRowState {
                            index: i,
                            weights: model::item_weights(data, i, stats),
                            selected: selected == Some(i),
                            first,
                            last,
                        },
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

fn item_row(ui: &mut Ui, item: &Value, names: Names, row: ItemRowState) -> Option<RowAction> {
    let mut action = None;
    let id = item.get("id").and_then(Value::as_str).unwrap_or("");
    let def = catalog::item(id);
    let icon = def.map_or("❓", |d| category_icon(d.category));
    let mut text = RichText::new(format!("{icon} {}", names.item(id)));
    if def.is_none() {
        text = text.color(colors::WARN);
    }
    let r = ui.selectable_label(row.selected, text);
    if r.clicked() {
        action = Some(RowAction::Select(row.index));
    }
    if let Some(weight) = row.weights.game {
        ui.weak(format!("{weight:.2} u"))
            .on_hover_text(names.text("Game item weight").to_string());
    }
    r.on_hover_ui(|ui| {
        ui.strong(names.item(id));
        ui.weak(format!("id: {id} · items[{}]", row.index));
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
            action = Some(RowAction::Delete(row.index));
        }
        if ui
            .small_button("⧉")
            .on_hover_text(names.text("Duplicate").to_string())
            .clicked()
        {
            action = Some(RowAction::Duplicate(row.index));
        }
        if ui
            .add_enabled(!row.last, egui::Button::new("⏷").small())
            .on_hover_text(names.text("Move down").to_string())
            .clicked()
        {
            action = Some(RowAction::Down(row.index));
        }
        if ui
            .add_enabled(!row.first, egui::Button::new("⏶").small())
            .on_hover_text(names.text("Move up").to_string())
            .clicked()
        {
            action = Some(RowAction::Up(row.index));
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
            action = Some(RowAction::ToggleFavourite(row.index));
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn component_names_are_shortened() {
        assert_eq!(component_short_name(details::WATER), "WaterContainerItem");
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
                container_capacity: None,
                container_max_item_weight: None,
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
                container_capacity: None,
                container_max_item_weight: None,
                container_encumbrance: None,
                liquid_capacity: None,
            },
        )]);

        assert!(add::do_add(&mut data, &mut state, &stats));
        assert_eq!(data["items"][0]["slot"], 0);
        assert_eq!(data["items"][0]["wearSlot"], Value::Null);
    }
}
