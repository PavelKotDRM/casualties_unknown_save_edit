//! Inventory tab: items grouped by location (held slot / worn slot) with the
//! container contents, an item details editor and an "add item" catalogue.

mod add;
mod details;
mod list;
mod location;
pub use location::location_name;
use location::{location_combo, location_icon};

use std::collections::HashMap;

use egui::{ProgressBar, RichText, Ui};
use serde_json::Value;

use super::{Names, card, category_icon, colors, quality_color};
use crate::catalog;
use crate::game_data::ItemStats;
use crate::json::{self, get_bool};
use crate::model::{self, Location};

pub use crate::model::placement::{
    HolderRules, default_location, holder_location_allowed, holder_rules, is_hand_slot,
    item_wear_slot, location_choices, missing_wear_limb,
};

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
    /// Most recent failure to construct or add an item.
    pub add_error: Option<String>,
    balance_rule: model::BalanceRule,
    move_favourites: bool,
    balance_result: Option<Result<usize, String>>,
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

/// Short name of a component type (`WaterContainerItem, Assembly-CSharp…`).
pub fn component_short_name(full: &str) -> &str {
    full.split(',').next().unwrap_or(full).trim()
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
        egui::ComboBox::from_id_salt("balance_rule")
            .selected_text(names.text(state.balance_rule.label()).into_owned())
            .show_ui(ui, |ui| {
                for rule in model::BalanceRule::ALL {
                    ui.selectable_value(&mut state.balance_rule, rule, names.text(rule.label()).into_owned());
                }
            });
        ui.checkbox(&mut state.move_favourites, names.text("Move favourites").into_owned());
        if ui.button(names.text("Balance containers").into_owned())
            .on_hover_text(names.text("Equipped items and compatible contents of specialized containers stay in place. Redistribute the remaining items only. If an optimum cannot be proven within the search limit, nothing is changed.").into_owned())
            .clicked()
        {
            let result = model::balance_containers(data, stats, state.balance_rule, !state.move_favourites);
            if result.as_ref().is_ok_and(|&moved| moved > 0) {
                state.selected = None;
                changed = true;
            }
            state.balance_result = Some(result);
        }
    });
    if let Some(result) = &state.balance_result {
        match result {
            Ok(moved) => {
                ui.colored_label(
                    colors::GOOD,
                    names
                        .text("{count} items moved")
                        .replace("{count}", &moved.to_string()),
                );
            }
            Err(error) => {
                ui.colored_label(colors::BAD, names.text(error).into_owned());
            }
        }
    }
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
                    action = list::item_list(ui, data, names, stats, state.selected);
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

    fn hold_slot_metadata() -> crate::game_data::metadata::Metadata {
        let names = [
            "Right hand",
            "Left hand",
            "Mouth",
            "Upper back",
            "Lower back",
            "Belt",
        ];
        crate::game_data::metadata::Metadata {
            hold_slots: names
                .into_iter()
                .map(|name| crate::schema::HoldSlotDef {
                    name,
                    locale_key: "",
                    icon: "",
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn component_names_are_shortened() {
        assert_eq!(component_short_name(details::WATER), "WaterContainerItem");
        assert_eq!(component_short_name("Foo"), "Foo");
    }

    #[test]
    fn slot_group_name_identifies_its_holder_item() {
        let _metadata = crate::game_data::runtime::scoped_for_test(hold_slot_metadata());
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
        let _metadata = crate::game_data::runtime::scoped_for_test(hold_slot_metadata());
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
        let _metadata =
            crate::game_data::runtime::scoped_for_test(crate::game_data::metadata::Metadata {
                items: vec![catalog::ItemDef {
                    id: "fixture_cap",
                    category: "wearable",
                    wear_slot: "head",
                    capacity: 0.0,
                }],
                ..Default::default()
            });
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
        assert!(add::do_add_fixture(&mut data, &mut state, &stats, names));
        assert_eq!(data["items"][0]["slot"], -1);
        assert_eq!(data["items"][0]["wearSlot"], "head");
    }
}
