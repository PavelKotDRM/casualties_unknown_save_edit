//! Item catalogue window and user-facing placement feedback.
use std::collections::HashMap;

use egui::{RichText, Ui};
use serde_json::{Value, json};

use super::{
    InventoryState, Names, colors, default_location, group_icon, holder_location_allowed,
    holder_rules, item_wear_slot, location_choices, location_icon, location_name, row_label,
};
use crate::catalog;
use crate::game_data::ItemStats;
use crate::model::{self, Location};

pub(super) fn add_window(
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
    let holder = holder_rules(&selected_id, stats);
    if state
        .add_location
        .as_ref()
        .is_some_and(|location| !holder_location_allowed(data, location, holder))
    {
        state.add_location = Some(default_location(holder));
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
                egui::ComboBox::from_id_salt("add_cat")
                    .selected_text(if state.add_category.is_empty() {
                        names.text("All categories").to_string()
                    } else {
                        let group = catalog::group(&state.add_category);
                        format!("{} {}", group.icon, names.text(group.label))
                    })
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut state.add_category,
                            String::new(),
                            names.text("All categories").to_string(),
                        );
                        for group in catalog::ITEM_GROUPS {
                            let count = catalog::ITEMS
                                .iter()
                                .filter(|d| d.group == group.id)
                                .count();
                            if count == 0 {
                                continue;
                            }
                            ui.selectable_value(
                                &mut state.add_category,
                                group.id.to_owned(),
                                format!("{} {} ({count})", group.icon, names.text(group.label)),
                            );
                        }
                    });
            });
            ui.separator();
            let needle = state.add_search.to_lowercase();
            let mut list: Vec<(&catalog::ItemDef, String)> = catalog::ITEMS
                .iter()
                .filter(|d| state.add_category.is_empty() || d.group == state.add_category)
                .map(|d| (d, names.item(d.id)))
                .filter(|(d, n)| {
                    needle.is_empty()
                        || d.id.contains(&needle)
                        || n.to_lowercase().contains(&needle)
                })
                .collect();
            list.sort_by_cached_key(|(d, name)| {
                (catalog::group_order(d.group), name.to_lowercase())
            });
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (d, name) in &list {
                        let r = ui
                            .selectable_label(
                                state.add_id == d.id,
                                format!("{} {name}   ", group_icon(d.group)),
                            )
                            .on_hover_text(format!(
                                "id: {}\n{} {}\n{} {}",
                                d.id,
                                names.text("Category:"),
                                names.text(catalog::group(d.group).label),
                                names.text("Game category:"),
                                d.category
                            ));
                        if r.clicked() {
                            state.add_id = d.id.to_owned();
                        }
                        if r.double_clicked() {
                            state.add_id = d.id.to_owned();
                            changed |= do_add(data, state, stats, names);
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
                let holder = holder_rules(&state.add_id, stats);
                let mut loc = state
                    .add_location
                    .clone()
                    .unwrap_or_else(|| default_location(holder));
                if !holder_location_allowed(data, &loc, holder) {
                    loc = default_location(holder);
                }
                add_location_combo(
                    ui,
                    "add_location",
                    data,
                    &mut loc,
                    holder_rules(&state.add_id, stats),
                    state.add_id.trim(),
                    names,
                    stats,
                );
                state.add_location = Some(loc);
                if let Some(slot) = item_wear_slot(&state.add_id, stats)
                    && ui.small_button(format!("Wear ({slot})")).clicked()
                {
                    state.add_location = Some(Location::Worn(slot.to_owned()));
                }
            });
            if let Some(loc) = &state.add_location {
                let (description, color, _) =
                    destination_status(data, loc, state.add_id.trim(), stats, names, None);
                ui.colored_label(color, description);
            }
            if let Some(error) = &state.add_error {
                ui.colored_label(colors::BAD, error);
            }
            let can_add = state.add_location.as_ref().is_some_and(|location| {
                destination_status(data, location, state.add_id.trim(), stats, names, None).2
            });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !state.add_id.trim().is_empty() && can_add,
                        egui::Button::new(format!("➕ {}", names.text("Add"))),
                    )
                    .clicked()
                {
                    changed |= do_add(data, state, stats, names);
                }
                if ui.button(names.text("Close").to_string()).clicked() {
                    close = true;
                }
            });
        });
    state.add_open = open && !close;
    changed
}

#[allow(clippy::too_many_arguments)] // independent pieces of UI state
fn add_location_combo(
    ui: &mut Ui,
    id: &str,
    data: &Value,
    current: &mut Location,
    holder: super::HolderRules,
    item_id: &str,
    names: Names,
    stats: &HashMap<String, ItemStats>,
) -> bool {
    let before = current.clone();
    egui::ComboBox::from_id_salt(id)
        .selected_text(location_name(current, names))
        .width(340.0)
        .height(400.0)
        .show_ui(ui, |ui| {
            for loc in location_choices(data, holder, current, false) {
                let (description, color, can_add) =
                    destination_status(data, &loc, item_id, stats, names, None);
                let text = format!(
                    "{} {} · {description}",
                    location_icon(&loc),
                    location_name(&loc, names)
                );
                ui.add_enabled_ui(can_add, |ui| {
                    ui.selectable_value(current, loc, RichText::new(text).color(color));
                });
            }
        });
    *current != before
}

pub(super) fn destination_status_for_move(
    data: &Value,
    location: &Location,
    item_index: usize,
    stats: &HashMap<String, ItemStats>,
    names: Names,
) -> (String, egui::Color32, bool) {
    let Some(item) = super::items(data).get(item_index) else {
        return rejection_status(
            names,
            crate::model::placement::Rejection::ItemDataUnavailable,
        );
    };
    if Location::of(item) == *location {
        return (
            names.text("Current location").to_string(),
            colors::NEUTRAL,
            true,
        );
    }
    let Some(item_id) = item.get("id").and_then(Value::as_str) else {
        return rejection_status(
            names,
            crate::model::placement::Rejection::ItemDataUnavailable,
        );
    };
    destination_status(data, location, item_id, stats, names, Some(item_index))
}

fn destination_status(
    data: &Value,
    location: &Location,
    item_id: &str,
    stats: &HashMap<String, ItemStats>,
    names: Names,
    item_index: Option<usize>,
) -> (String, egui::Color32, bool) {
    match crate::model::placement::check_destination(data, stats, item_id, location, item_index) {
        Ok(destination) => destination_status_text(names, destination, item_index.is_none()),
        Err(rejection) => rejection_status(names, rejection),
    }
}

fn destination_status_text(
    names: Names,
    destination: crate::model::placement::Destination,
    adding: bool,
) -> (String, egui::Color32, bool) {
    match destination {
        crate::model::placement::Destination::Empty => (
            names
                .text(if adding {
                    "New item will be held/worn"
                } else {
                    "Empty location"
                })
                .to_string(),
            colors::ACCENT,
            true,
        ),
        crate::model::placement::Destination::Container {
            holder_id,
            remaining,
            capacity,
        } => (
            names
                .text("Inside {item}: {remaining}/{capacity} u left")
                .replace("{item}", &names.item(&holder_id))
                .replace("{remaining}", &format!("{remaining:.1}"))
                .replace("{capacity}", &format!("{capacity:.1}")),
            colors::ACCENT,
            true,
        ),
    }
}

fn rejection_status(
    names: Names,
    rejection: crate::model::placement::Rejection,
) -> (String, egui::Color32, bool) {
    use crate::model::placement::Rejection;

    let text = match &rejection {
        Rejection::ItemDataUnavailable => names.text("Item data unavailable").into_owned(),
        Rejection::ContainerDataUnavailable(item) if item.is_empty() => {
            names.text("Container data unavailable").into_owned()
        }
        Rejection::ContainerDataUnavailable(item) => names
            .text("Container data unavailable for {item}")
            .replace("{item}", &names.item(item)),
        Rejection::OccupiedByNonContainer(item) => names
            .text("Occupied by {item} (not a container)")
            .replace("{item}", &names.item(item)),
        Rejection::MissingWearLimb(limb) => names
            .text("Cannot be worn: {limb} is dismembered")
            .replace("{limb}", &names.limb(*limb)),
        Rejection::ClothingNotHoldable => names
            .text("Clothing can't be held; wear it or store it in a container")
            .into_owned(),
        Rejection::HandsOnly => names.text("Can only be held in hands").into_owned(),
        Rejection::CannotWear => names.text("Item cannot be worn here").into_owned(),
        Rejection::FilledContainer => names
            .text("A container with items can't be stored in another container")
            .into_owned(),
        Rejection::WeightLimitUnavailable(item) => names
            .text("Inside {item}: weight limits unavailable")
            .replace("{item}", &names.item(item)),
        Rejection::RestrictionsUnavailable(item) => names
            .text("Inside {item}: restrictions unavailable")
            .replace("{item}", &names.item(item)),
        Rejection::ItemTypeUnavailable(item) => names
            .text("Cannot check item type for {item}")
            .replace("{item}", &names.item(item)),
        Rejection::ItemTypeNotAccepted(item) => names
            .text("Item type not accepted by {item}")
            .replace("{item}", &names.item(item)),
        Rejection::ItemWeightUnavailable(item) => names
            .text("Cannot verify item weight for {item}")
            .replace("{item}", &names.item(item)),
        Rejection::ItemTooHeavy(item) => names
            .text("Item exceeds max weight per item for {item}")
            .replace("{item}", &names.item(item)),
        Rejection::CapacityUnavailable(item) => names
            .text("Inside {item}: capacity unavailable")
            .replace("{item}", &names.item(item)),
        Rejection::OverCapacity(item) => names
            .text("Inside {item}: over capacity")
            .replace("{item}", &names.item(item)),
        Rejection::Full(item) => names
            .text("Inside {item}: full")
            .replace("{item}", &names.item(item)),
        Rejection::NotEnoughCapacity(item) => names
            .text("Not enough remaining capacity in {item}")
            .replace("{item}", &names.item(item)),
    };
    let color = if rejection.is_metadata_warning() {
        colors::WARN
    } else {
        colors::BAD
    };
    (text, color, false)
}

pub(super) fn do_add(
    data: &mut Value,
    state: &mut InventoryState,
    stats: &HashMap<String, ItemStats>,
    names: Names,
) -> bool {
    do_add_with(data, state, stats, names, model::new_item)
}

fn do_add_with(
    data: &mut Value,
    state: &mut InventoryState,
    stats: &HashMap<String, ItemStats>,
    names: Names,
    create: impl FnOnce(&str, &Location) -> Result<Value, String>,
) -> bool {
    let holder = holder_rules(state.add_id.trim(), stats);
    let mut loc = state
        .add_location
        .clone()
        .unwrap_or_else(|| default_location(holder));
    if !holder_location_allowed(data, &loc, holder) {
        loc = default_location(holder);
    }
    if !destination_status(data, &loc, state.add_id.trim(), stats, names, None).2 {
        return false;
    }
    let item = match create(state.add_id.trim(), &loc) {
        Ok(item) => item,
        Err(error) => {
            state.add_error = Some(error);
            return false;
        }
    };
    if data.get("items").and_then(Value::as_array).is_none() {
        state.add_error = Some("This save has no items array.".to_owned());
        return false;
    }
    match model::add_item(data, item, json!({})) {
        Some(i) => {
            state.selected = Some(i);
            state.add_error = None;
            true
        }
        None => {
            state.add_error = Some("This save has no items array.".to_owned());
            false
        }
    }
}

#[cfg(test)]
pub(super) fn do_add_fixture(
    data: &mut Value,
    state: &mut InventoryState,
    stats: &HashMap<String, ItemStats>,
    names: Names,
) -> bool {
    do_add_with(data, state, stats, names, |id, location| {
        let mut item = json!({"id": id, "condition": 1.0});
        location.apply(&mut item);
        Ok(item)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;
    use crate::ui::InterfaceLanguage;

    fn test_item_stats(tags: &[&str]) -> ItemStats {
        ItemStats {
            category: "medical".to_owned(),
            weight: 1.0,
            scale_weight_with_condition: false,
            wearable: false,
            wear_slot: None,
            armor: None,
            insulation: None,
            item_tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            container_capacity: None,
            container_max_item_weight: None,
            container_encumbrance: None,
            container_tag_restriction: None,
            liquid_capacity: None,
            only_hold_in_hands: false,
            wearable_can_be_held: false,
            desired_wear_limb: None,
            destroy_at_zero_condition: false,
        }
    }

    fn names() -> Names<'static> {
        Names {
            locale: None,
            interface_language: InterfaceLanguage::English,
        }
    }

    #[test]
    fn constructor_failure_is_visible_and_does_not_change_the_inventory() {
        let mut data = json!({"items": []});
        let original = data.clone();
        let mut state = InventoryState {
            add_id: "item".to_owned(),
            ..Default::default()
        };
        let stats = HashMap::new();
        assert!(!do_add_with(
            &mut data,
            &mut state,
            &stats,
            names(),
            |_, _| { Err("Missing item defaults".to_owned()) }
        ));
        assert_eq!(state.add_error.as_deref(), Some("Missing item defaults"));
        assert_eq!(data, original);
        assert!(do_add_fixture(&mut data, &mut state, &stats, names()));
        assert!(state.add_error.is_none());
    }

    #[test]
    fn adding_to_a_container_rejects_items_without_a_matching_tag() {
        let data = json!({
            "items": [{"id": "ammo_case", "slot": 0}]
        });
        let mut container = test_item_stats(&[]);
        container.container_capacity = Some(10.0);
        container.container_max_item_weight = Some(5.0);
        container.container_encumbrance = Some(0.5);
        container.container_tag_restriction = Some(HashSet::from(["bullet".to_owned()]));
        let stats = HashMap::from([
            ("ammo_case".to_owned(), container),
            ("syringe".to_owned(), test_item_stats(&["medical"])),
        ]);
        let mut state = InventoryState {
            add_id: "syringe".to_owned(),
            add_location: Some(Location::Slot(0)),
            ..Default::default()
        };
        let mut data = data;

        assert!(!do_add_fixture(&mut data, &mut state, &stats, names()));
        assert_eq!(data["items"].as_array().unwrap().len(), 1);
        let status =
            destination_status(&data, &Location::Slot(0), "syringe", &stats, names(), None);
        assert!(!status.2);
    }

    #[test]
    fn adding_to_a_container_allows_a_matching_tag() {
        let mut data = json!({
            "items": [{"id": "ammo_case", "slot": 0}]
        });
        let mut container = test_item_stats(&[]);
        container.container_capacity = Some(10.0);
        container.container_max_item_weight = Some(5.0);
        container.container_encumbrance = Some(0.5);
        container.container_tag_restriction = Some(HashSet::from(["bullet".to_owned()]));
        let stats = HashMap::from([
            ("ammo_case".to_owned(), container),
            ("round".to_owned(), test_item_stats(&["bullet"])),
        ]);
        let mut state = InventoryState {
            add_id: "round".to_owned(),
            add_location: Some(Location::Slot(0)),
            ..Default::default()
        };

        assert!(do_add_fixture(&mut data, &mut state, &stats, names()));
        assert_eq!(data["items"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn adding_checks_per_item_weight_and_remaining_capacity() {
        let mut container = test_item_stats(&[]);
        container.container_capacity = Some(10.0);
        container.container_max_item_weight = Some(0.5);
        container.container_encumbrance = Some(0.5);
        container.container_tag_restriction = Some(HashSet::new());
        let stats = HashMap::from([
            ("case".to_owned(), container.clone()),
            ("round".to_owned(), test_item_stats(&[])),
        ]);
        let data = json!({"items": [{"id": "case", "slot": 0}]});
        assert!(!destination_status(&data, &Location::Slot(0), "round", &stats, names(), None).2);

        container.container_max_item_weight = Some(10.0);
        let stats = HashMap::from([
            ("case".to_owned(), container),
            (
                "heavy".to_owned(),
                ItemStats {
                    weight: 9.5,
                    ..test_item_stats(&[])
                },
            ),
            ("round".to_owned(), test_item_stats(&[])),
        ]);
        let data = json!({"items": [
            {"id": "case", "slot": 0},
            {"id": "heavy", "slot": 0}
        ]});
        assert!(!destination_status(&data, &Location::Slot(0), "round", &stats, names(), None).2);
    }

    #[test]
    fn moving_an_item_into_an_incompatible_container_is_rejected() {
        let mut container = test_item_stats(&[]);
        container.container_capacity = Some(10.0);
        container.container_max_item_weight = Some(5.0);
        container.container_encumbrance = Some(0.5);
        container.container_tag_restriction = Some(HashSet::from(["bullet".to_owned()]));
        let stats = HashMap::from([
            ("case".to_owned(), container),
            ("syringe".to_owned(), test_item_stats(&["medical"])),
        ]);
        let data = json!({"items": [
            {"id": "syringe", "slot": 0},
            {"id": "case", "slot": 1}
        ]});

        assert!(!destination_status_for_move(&data, &Location::Slot(1), 0, &stats, names()).2);
    }

    #[test]
    fn hands_only_items_cannot_be_added_to_an_empty_mouth_or_back() {
        let stats = HashMap::from([(
            "toolbox".to_owned(),
            ItemStats {
                only_hold_in_hands: true,
                ..test_item_stats(&[])
            },
        )]);
        let mut data = json!({"items": []});
        let status =
            destination_status(&data, &Location::Slot(2), "toolbox", &stats, names(), None);
        assert_eq!(
            (status.0.as_str(), status.2),
            ("Can only be held in hands", false)
        );

        let mut state = InventoryState {
            add_id: "toolbox".to_owned(),
            add_location: Some(Location::Slot(1)),
            ..Default::default()
        };
        assert!(do_add_fixture(&mut data, &mut state, &stats, names()));
        assert!(!destination_status_for_move(&data, &Location::Slot(4), 0, &stats, names()).2);
    }

    #[test]
    fn clothing_can_only_be_worn_or_stored_in_a_container() {
        let mut bag = test_item_stats(&[]);
        bag.container_capacity = Some(20.0);
        bag.container_max_item_weight = Some(5.0);
        bag.container_encumbrance = Some(0.5);
        bag.container_tag_restriction = Some(HashSet::new());
        let coat = ItemStats {
            wearable: true,
            wear_slot: Some("torso".to_owned()),
            ..test_item_stats(&[])
        };
        let stats = HashMap::from([
            ("bigpack".to_owned(), bag),
            ("coat".to_owned(), coat.clone()),
            (
                "autopump".to_owned(),
                ItemStats {
                    wearable_can_be_held: true,
                    ..coat
                },
            ),
        ]);
        let data = json!({"items": [{"id": "bigpack", "slot": 3}]});
        let status = destination_status(&data, &Location::Slot(0), "coat", &stats, names(), None);
        assert_eq!(
            (status.0.as_str(), status.2),
            (
                "Clothing can't be held; wear it or store it in a container",
                false
            )
        );
        let check = |location: Location, id: &str| {
            destination_status(&data, &location, id, &stats, names(), None).2
        };
        assert!(check(Location::Slot(3), "coat"));
        assert!(check(Location::Worn("torso".to_owned()), "coat"));
        assert!(check(Location::Slot(0), "autopump"));

        let mut data = data.clone();
        let mut state = InventoryState {
            add_id: "coat".to_owned(),
            ..Default::default()
        };
        assert!(do_add_fixture(&mut data, &mut state, &stats, names()));
        assert_eq!(data["items"][1]["wearSlot"], "torso");
    }

    #[test]
    fn clothing_cannot_be_worn_on_a_dismembered_limb() {
        let _metadata =
            crate::game_data::runtime::scoped_for_test(crate::game_data::metadata::Metadata {
                limbs: vec![crate::schema::LimbDef {
                    object: "Head",
                    name: "Head",
                }],
                ..Default::default()
            });
        let cap = ItemStats {
            wearable: true,
            wear_slot: Some("head".to_owned()),
            desired_wear_limb: Some("Head".to_owned()),
            ..test_item_stats(&[])
        };
        let stats = HashMap::from([("cap".to_owned(), cap)]);
        let mut data = json!({"items": [], "limbs": [{"dismembered": true}]});
        let head = Location::Worn("head".to_owned());
        let status = destination_status(&data, &head, "cap", &stats, names(), None);
        assert_eq!(
            (status.0.as_str(), status.2),
            ("Cannot be worn: Head is dismembered", false)
        );
        data["limbs"][0]["dismembered"] = json!(false);
        assert!(destination_status(&data, &head, "cap", &stats, names(), None).2);
    }

    #[test]
    fn a_filled_container_cannot_be_moved_into_another_container() {
        let mut bag = test_item_stats(&[]);
        bag.container_capacity = Some(20.0);
        bag.container_max_item_weight = Some(5.0);
        bag.container_encumbrance = Some(0.5);
        bag.container_tag_restriction = Some(HashSet::new());
        let stats = HashMap::from([
            ("bag".to_owned(), bag),
            ("rock".to_owned(), test_item_stats(&[])),
        ]);
        let data = json!({"items": [
            {"id": "bag", "slot": 0},
            {"id": "rock", "slot": 0},
            {"id": "bag", "slot": 1}
        ]});
        let status = destination_status_for_move(&data, &Location::Slot(1), 0, &stats, names());
        assert!(!status.2);
        assert_eq!(
            status.0,
            "A container with items can't be stored in another container"
        );
        assert!(destination_status_for_move(&data, &Location::Slot(1), 1, &stats, names()).2);
    }
}
