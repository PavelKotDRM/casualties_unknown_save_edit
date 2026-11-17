//! Inventory tab: items grouped by location (held slot / worn slot) with the
//! container contents, an item details editor and an "add item" catalogue.

// FIXME: ~790 lines; the holder/placement rules (`HolderRules`, `holder_rules`,
// `missing_wear_limb`, `default_location`, `location_choices`, `holder_location_allowed`,
// `item_wear_slot`, `is_hand_slot`) are pure save-document logic but live in the UI module and
// are re-imported by `add.rs`/`details.rs` through `super::*`. Move them into
// `model::inventory` (or a new `model/placement.rs`) with their tests, and move `location_name`/
// `location_icon`/`location_combo` into a small `ui/inventory/location.rs`. `show` (~120 lines)
// and `item_list` (~130 lines) should be split so the per-group header, the nested container
// rows and the drag/reorder handling are separate functions.

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
pub fn location_name(loc: &Location, names: Names) -> String {
    let language = names.interface_language;
    match loc {
        Location::Slot(s) => names.hold_slot(*s),
        Location::Worn(w) if w.is_empty() => language
            .text("Unassigned (slot -1, no wear slot)")
            .into_owned(),
        Location::Worn(w) => format!("{}: {w}", language.text("Worn")),
    }
}

fn group_location_name(group: &model::ItemGroup, items: &[Value], names: Names) -> String {
    let location = location_name(&group.location, names);
    if !matches!(&group.location, Location::Slot(_)) {
        return location;
    }

    let Some(item) = group.indices.first().and_then(|index| items.get(*index)) else {
        return location;
    };
    let name = item
        .get("label")
        .and_then(Value::as_str)
        .filter(|label| !label.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            item.get("id")
                .and_then(Value::as_str)
                .map(|id| names.item(id))
        });
    name.map_or(location.clone(), |name| format!("{location} · {name}"))
}

fn location_icon(loc: &Location) -> &'static str {
    match loc {
        Location::Slot(s) => usize::try_from(*s)
            .ok()
            .and_then(|i| crate::schema::HOLD_SLOTS.get(i))
            .map_or("❓", |def| def.icon),
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

/// What an item needs to become the holder of an empty location.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct HolderRules<'a> {
    /// Wear slot the item can be worn in.
    wear_slot: Option<&'a str>,
    /// The game only lets the item occupy a hand slot.
    hands_only: bool,
    /// Clothing that can only be worn or stored, never held in a body slot.
    not_holdable: bool,
    /// Limb object the item is worn on; it cannot be worn while that limb is missing.
    wear_limb: Option<&'a str>,
}

fn holder_rules<'a>(id: &str, stats: &'a HashMap<String, ItemStats>) -> HolderRules<'a> {
    HolderRules {
        wear_slot: item_wear_slot(id, stats),
        hands_only: stats.get(id).is_some_and(|item| item.only_hold_in_hands),
        not_holdable: match stats.get(id) {
            Some(item) => item.wearable && !item.wearable_can_be_held,
            None => item_wear_slot(id, stats).is_some(),
        },
        wear_limb: stats
            .get(id)
            .and_then(|item| item.desired_wear_limb.as_deref()),
    }
}

/// Index of the limb a wearable goes on when that limb is dismembered.
fn missing_wear_limb(data: &Value, holder: HolderRules) -> Option<usize> {
    let index = crate::schema::LIMBS
        .iter()
        .position(|limb| Some(limb.object) == holder.wear_limb)?;
    data.get("limbs")
        .and_then(|limbs| limbs.get(index))
        .and_then(|limb| limb.get("dismembered"))
        .and_then(Value::as_bool)
        .filter(|missing| *missing)
        .map(|_| index)
}

/// Where a new or reset item should go by default: a hand, or its wear slot
/// for clothing that cannot be held.
fn default_location(holder: HolderRules) -> Location {
    match holder.wear_slot {
        Some(slot) if holder.not_holdable => Location::Worn(slot.to_owned()),
        _ => Location::Slot(0),
    }
}

fn is_hand_slot(slot: i64) -> bool {
    matches!(slot, 0 | 1)
}

/// Locations offered in choosers, with only compatible wear slots.
fn location_choices(
    data: &Value,
    holder: HolderRules,
    current: &Location,
    preserve_current: bool,
) -> Vec<Location> {
    let mut out: Vec<Location> = model::group_items(data)
        .into_iter()
        .map(|g| g.location)
        .filter(|location| holder_location_allowed(data, location, holder))
        .collect();
    for s in 0..crate::schema::HOLD_SLOTS.len() as i64 {
        let l = Location::Slot(s);
        if !out.contains(&l) {
            out.push(l);
        }
    }
    if let Some(slot) = holder.wear_slot {
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

/// Whether a location may be targeted: an occupied location stores the item
/// inside its holder, while an empty one requires the item to be holdable
/// there (a matching wear slot on a limb that is not dismembered, a hand for
/// hands-only items, and never a body slot for clothing).
fn holder_location_allowed(data: &Value, location: &Location, holder: HolderRules) -> bool {
    let occupied = || {
        model::group_items(data)
            .iter()
            .any(|group| &group.location == location)
    };
    match location {
        Location::Slot(slot) => {
            (!holder.not_holdable && (!holder.hands_only || is_hand_slot(*slot))) || occupied()
        }
        Location::Worn(slot) => {
            (holder.wear_slot == Some(slot.as_str()) && missing_wear_limb(data, holder).is_none())
                || (!slot.is_empty() && occupied())
        }
    }
}

#[allow(clippy::too_many_arguments)] // independent pieces of UI state
fn location_combo(
    ui: &mut Ui,
    id: &str,
    data: &Value,
    current: &mut Location,
    holder: HolderRules,
    preserve_current: bool,
    names: Names,
    stats: &HashMap<String, ItemStats>,
    item_index: usize,
) -> bool {
    let before = current.clone();
    egui::ComboBox::from_id_salt(id)
        .selected_text(location_name(current, names))
        .width(240.0)
        .height(400.0)
        .show_ui(ui, |ui| {
            for loc in location_choices(data, holder, current, preserve_current) {
                let (description, color, can_move) =
                    add::destination_status_for_move(data, &loc, item_index, stats, names);
                let text = format!(
                    "{} {} · {description}",
                    location_icon(&loc),
                    location_name(&loc, names)
                );
                ui.add_enabled_ui(can_move, |ui| {
                    ui.selectable_value(current, loc, RichText::new(text).color(color));
                });
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
            let header = ui.label(
                RichText::new(format!(
                    "{}  {}",
                    location_icon(&group.location),
                    group_location_name(&group, all, names)
                ))
                .strong()
                .color(colors::ACCENT),
            );
            if matches!(&group.location, Location::Slot(_)) {
                header.on_hover_text(
                    names
                        .text("The first item in this slot is its holder; any following items are stored inside it.")
                        .to_string(),
                );
            }
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
                if first
                    && let Some(id) = all[i].get("id").and_then(Value::as_str)
                    && stats
                        .get(id)
                        .is_some_and(|item_stats| item_stats.container_capacity.is_some())
                {
                    if let Some(space) = model::container_space(data, i, stats) {
                        ui.horizontal(|ui| {
                            ui.add_space(22.0);
                            let (label, color) = if !space.complete {
                                (
                                    names.text("Container capacity unavailable").to_string(),
                                    colors::WARN,
                                )
                            } else if space.over_capacity > 0.0 {
                                (
                                    names.text("Over capacity by {weight} u").replace(
                                        "{weight}",
                                        &format!("{:.2}", space.over_capacity),
                                    ),
                                    colors::BAD,
                                )
                            } else if space.remaining <= f32::EPSILON {
                                (names.text("Full").to_string(), colors::BAD)
                            } else {
                                (
                                    names
                                        .text("Space left: {weight} u")
                                        .replace("{weight}", &format!("{:.2}", space.remaining)),
                                    colors::ACCENT,
                                )
                            };
                            ui.colored_label(color, label);
                            if space.complete {
                                let fill = if space.capacity > 0.0 {
                                    (space.used / space.capacity).clamp(0.0, 1.0)
                                } else {
                                    1.0
                                };
                                ui.add(
                                    ProgressBar::new(fill)
                                        .desired_width(120.0)
                                        .show_percentage()
                                        .fill(color),
                                );
                            }
                        });
                    } else {
                        ui.horizontal(|ui| {
                            ui.add_space(22.0);
                            ui.colored_label(
                                colors::WARN,
                                names.text("Container capacity unavailable").to_string(),
                            );
                        });
                    }
                }
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
    use crate::ui::InterfaceLanguage;
    use serde_json::json;

    #[test]
    fn component_names_are_shortened() {
        assert_eq!(component_short_name(details::WATER), "WaterContainerItem");
        assert_eq!(component_short_name("Foo"), "Foo");
    }

    #[test]
    fn slot_group_name_identifies_its_holder_item() {
        let data = json!({
            "items": [
                {"id": "bigpack", "slot": 3},
                {"id": "bandage", "slot": 3},
                {"id": "bandage", "slot": 9}
            ]
        });
        let groups = model::group_items(&data);
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::English,
        };

        assert_eq!(
            group_location_name(&groups[0], items(&data), names),
            "Upper back · Bigpack"
        );
        assert_eq!(
            group_location_name(&groups[1], items(&data), names),
            "Unknown slot 9 · Bandage"
        );
        assert_eq!(location_name(&Location::Slot(0), names), "Right hand");
        assert_eq!(location_name(&Location::Slot(2), names), "Mouth");
    }

    fn wear(slot: Option<&str>) -> HolderRules<'_> {
        HolderRules {
            wear_slot: slot,
            ..HolderRules::default()
        }
    }

    #[test]
    fn hands_only_items_need_a_hand_unless_stored_in_a_container() {
        let data = json!({"items": [{"id": "bigpack", "slot": 3}]});
        let holder = HolderRules {
            hands_only: true,
            ..HolderRules::default()
        };
        assert!(holder_location_allowed(&data, &Location::Slot(0), holder));
        assert!(holder_location_allowed(&data, &Location::Slot(1), holder));
        assert!(!holder_location_allowed(&data, &Location::Slot(2), holder));
        assert!(!holder_location_allowed(&data, &Location::Slot(5), holder));
        // An occupied back slot stores the item inside the backpack.
        assert!(holder_location_allowed(&data, &Location::Slot(3), holder));
    }

    #[test]
    fn location_choices_offer_only_real_hold_slots() {
        let choices = location_choices(
            &json!({"items": []}),
            HolderRules::default(),
            &Location::Slot(0),
            false,
        );
        assert!(choices.contains(&Location::Slot(5)));
        assert!(!choices.contains(&Location::Slot(6)));
    }

    #[test]
    fn location_choices_only_offer_the_supported_empty_wear_slot() {
        let data = json!({
            "items": [
                {"id": "coat", "slot": -1, "wearSlot": "torso"}
            ]
        });
        let choices = location_choices(&data, wear(Some("head")), &Location::Slot(0), false);
        assert!(choices.contains(&Location::Worn("head".to_owned())));
        assert!(choices.contains(&Location::Worn("torso".to_owned())));
        assert!(!choices.contains(&Location::Worn("back".to_owned())));
    }

    #[test]
    fn occupied_wear_slots_are_offered_as_container_destinations() {
        let data = json!({
            "items": [
                {"id": "holster", "slot": -1, "wearSlot": "thigh"},
                {"id": "bandage", "slot": 0}
            ]
        });
        let choices = location_choices(&data, HolderRules::default(), &Location::Slot(0), false);
        assert!(choices.contains(&Location::Worn("thigh".to_owned())));
        assert!(!holder_location_allowed(
            &data,
            &Location::Worn("head".to_owned()),
            HolderRules::default()
        ));
    }

    #[test]
    fn existing_incompatible_wear_slot_remains_visible_for_repair() {
        let data = json!({
            "items": [
                {"id": "coat", "slot": -1, "wearSlot": "torso"}
            ]
        });
        let current = Location::Worn("torso".to_owned());
        let choices = location_choices(&data, wear(Some("head")), &current, true);
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
                item_tags: Default::default(),
                container_capacity: None,
                container_max_item_weight: None,
                container_encumbrance: None,
                container_tag_restriction: None,
                liquid_capacity: None,
                only_hold_in_hands: false,
                wearable_can_be_held: false,
                desired_wear_limb: None,
                destroy_at_zero_condition: false,
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
                item_tags: Default::default(),
                container_capacity: None,
                container_max_item_weight: None,
                container_encumbrance: None,
                container_tag_restriction: None,
                liquid_capacity: None,
                only_hold_in_hands: false,
                wearable_can_be_held: false,
                desired_wear_limb: None,
                destroy_at_zero_condition: false,
            },
        )]);

        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::English,
        };
        assert!(add::do_add(&mut data, &mut state, &stats, names));
        assert_eq!(data["items"][0]["slot"], -1);
        assert_eq!(data["items"][0]["wearSlot"], "head");
    }
}
