use std::collections::HashMap;

use egui::Ui;
use serde_json::{Value, json};

use super::{InventoryState, Names, category_icon, item_wear_slot, location_combo, row_label};
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

pub(super) fn do_add(
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
