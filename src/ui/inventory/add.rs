// FIXME: ~750 lines with no module docs; `add_window` is ~150 lines and `destination_status`
// ~215 lines of nested `if`/`match` deciding whether an item fits a destination (holder rules, wear
// slot, container tags, weight, remaining capacity). Extract the validation into a UI-free
// `model::placement::check_destination(data, stats, item, location) -> Result<(), Rejection>`
// enum, map `Rejection` to text in one `fn rejection_text(names, Rejection)`, and have both
// `destination_status` and `destination_status_for_move` call it. Move the ~230 lines of tests
// with the extracted logic so they no longer need to construct egui `Names`.
use std::collections::HashMap;

use egui::{RichText, Ui};
use serde_json::{Value, json};

use super::{
    InventoryState, Names, category_icon, colors, default_location, holder_location_allowed,
    holder_rules, item_wear_slot, location_choices, location_icon, location_name,
    missing_wear_limb, row_label,
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
        return (
            names.text("Item data unavailable").to_string(),
            colors::WARN,
            false,
        );
    };
    if Location::of(item) == *location {
        return (
            names.text("Current location").to_string(),
            colors::NEUTRAL,
            true,
        );
    }
    if !model::group_items(data)
        .iter()
        .any(|group| &group.location == location)
    {
        let holder = item
            .get("id")
            .and_then(Value::as_str)
            .map(|id| holder_rules(id, stats))
            .unwrap_or_default();
        if !holder_location_allowed(data, location, holder) {
            return (
                not_holdable_text(data, location, holder, names),
                colors::BAD,
                false,
            );
        }
        return (
            names.text("Empty location").to_string(),
            colors::ACCENT,
            true,
        );
    }
    let Some(item_id) = item.get("id").and_then(Value::as_str) else {
        return (
            names.text("Item data unavailable").to_string(),
            colors::WARN,
            false,
        );
    };
    destination_status(data, location, item_id, stats, names, Some(item_index))
}

fn not_holdable_text(
    data: &Value,
    location: &Location,
    holder: super::HolderRules,
    names: Names,
) -> String {
    if let Location::Worn(_) = location
        && let Some(limb) = missing_wear_limb(data, holder)
    {
        return names
            .text("Cannot be worn: {limb} is dismembered")
            .replace("{limb}", &names.limb(limb));
    }
    match location {
        Location::Slot(_) if holder.not_holdable => names
            .text("Clothing can't be held; wear it or store it in a container")
            .into_owned(),
        Location::Slot(_) => names.text("Can only be held in hands").into_owned(),
        Location::Worn(_) => names.text("Item cannot be worn here").into_owned(),
    }
}

fn destination_status(
    data: &Value,
    location: &Location,
    item_id: &str,
    stats: &HashMap<String, ItemStats>,
    names: Names,
    item_index: Option<usize>,
) -> (String, egui::Color32, bool) {
    let Some(group) = model::group_items(data)
        .into_iter()
        .find(|group| &group.location == location)
    else {
        let holder = holder_rules(item_id, stats);
        if !holder_location_allowed(data, location, holder) {
            return (
                not_holdable_text(data, location, holder, names),
                colors::BAD,
                false,
            );
        }
        return (
            names.text("New item will be held/worn").to_string(),
            colors::ACCENT,
            true,
        );
    };
    let Some(&root_index) = group.indices.first() else {
        return (
            names.text("Container data unavailable").to_string(),
            colors::WARN,
            false,
        );
    };
    let Some(root_id) = super::items(data)
        .get(root_index)
        .and_then(|item| item.get("id"))
        .and_then(Value::as_str)
    else {
        return (
            names.text("Container data unavailable").to_string(),
            colors::WARN,
            false,
        );
    };
    let root_name = names.item(root_id);
    let moving_filled_container = item_index.is_some_and(|index| {
        model::group_items(data)
            .iter()
            .any(|g| g.indices.len() > 1 && g.indices.first() == Some(&index))
    });
    if moving_filled_container {
        return (
            names
                .text("A container with items can't be stored in another container")
                .to_string(),
            colors::BAD,
            false,
        );
    }
    if let Some(container_stats) = stats.get(root_id) {
        let Some(capacity) = container_stats.container_capacity else {
            let known_non_container =
                catalog::item(root_id).is_some_and(|item| item.category != "container");
            let status = if known_non_container {
                names
                    .text("Occupied by {item} (not a container)")
                    .replace("{item}", &root_name)
            } else {
                names
                    .text("Container data unavailable for {item}")
                    .replace("{item}", &root_name)
            };
            return (status, colors::WARN, false);
        };
        let Some(max_item_weight) = container_stats
            .container_max_item_weight
            .filter(|weight| weight.is_finite() && *weight >= 0.0)
        else {
            return (
                names
                    .text("Inside {item}: weight limits unavailable")
                    .replace("{item}", &root_name),
                colors::WARN,
                false,
            );
        };
        let Some(restrictions) = container_stats.container_tag_restriction.as_ref() else {
            return (
                names
                    .text("Inside {item}: restrictions unavailable")
                    .replace("{item}", &root_name),
                colors::WARN,
                false,
            );
        };
        if !restrictions.is_empty() {
            let Some(item_stats) = stats.get(item_id) else {
                return (
                    names
                        .text("Cannot check item type for {item}")
                        .replace("{item}", &root_name),
                    colors::WARN,
                    false,
                );
            };
            if container_stats.accepts_item(item_stats) == Some(false) {
                return (
                    names
                        .text("Item type not accepted by {item}")
                        .replace("{item}", &root_name),
                    colors::BAD,
                    false,
                );
            }
        }
        let item_weight = if let Some(index) = item_index {
            let weights = model::item_weights(data, index, stats);
            weights.game_complete.then_some(weights.game).flatten()
        } else {
            stats.get(item_id).and_then(|item_stats| {
                item_stats
                    .liquid_capacity
                    .is_none()
                    .then(|| item_stats.total_weight(1.0))
            })
        };
        let Some(item_weight) = item_weight.filter(|weight| weight.is_finite() && *weight >= 0.0)
        else {
            return (
                names
                    .text("Cannot verify item weight for {item}")
                    .replace("{item}", &root_name),
                colors::WARN,
                false,
            );
        };
        if item_weight > max_item_weight + f32::EPSILON {
            return (
                names
                    .text("Item exceeds max weight per item for {item}")
                    .replace("{item}", &root_name),
                colors::BAD,
                false,
            );
        }
        let Some(space) =
            model::container_space(data, root_index, stats).filter(|space| space.complete)
        else {
            return (
                names
                    .text("Inside {item}: capacity unavailable")
                    .replace("{item}", &root_name),
                colors::WARN,
                false,
            );
        };
        let (text, color, can_add) = if space.over_capacity > 0.0 {
            (
                names
                    .text("Inside {item}: over capacity")
                    .replace("{item}", &root_name),
                colors::BAD,
                false,
            )
        } else if space.remaining <= f32::EPSILON {
            (
                names
                    .text("Inside {item}: full")
                    .replace("{item}", &root_name),
                colors::BAD,
                false,
            )
        } else if item_weight > space.remaining + f32::EPSILON {
            (
                names
                    .text("Not enough remaining capacity in {item}")
                    .replace("{item}", &root_name),
                colors::BAD,
                false,
            )
        } else {
            (
                names
                    .text("Inside {item}: {remaining}/{capacity} u left")
                    .replace("{item}", &root_name)
                    .replace("{remaining}", &format!("{:.1}", space.remaining))
                    .replace("{capacity}", &format!("{:.1}", capacity)),
                colors::ACCENT,
                true,
            )
        };
        return (text, color, can_add);
    }

    let known_non_container = stats.contains_key(root_id)
        || catalog::item(root_id).is_some_and(|item| item.category != "container");
    if known_non_container {
        (
            names
                .text("Occupied by {item} (not a container)")
                .replace("{item}", &root_name),
            colors::WARN,
            false,
        )
    } else {
        (
            names
                .text("Container data unavailable for {item}")
                .replace("{item}", &root_name),
            colors::WARN,
            false,
        )
    }
}

pub(super) fn do_add(
    data: &mut Value,
    state: &mut InventoryState,
    stats: &HashMap<String, ItemStats>,
    names: Names,
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

        assert!(!do_add(&mut data, &mut state, &stats, names()));
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

        assert!(do_add(&mut data, &mut state, &stats, names()));
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
        assert!(do_add(&mut data, &mut state, &stats, names()));
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
        assert!(do_add(&mut data, &mut state, &stats, names()));
        assert_eq!(data["items"][1]["wearSlot"], "torso");
    }

    #[test]
    fn clothing_cannot_be_worn_on_a_dismembered_limb() {
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
