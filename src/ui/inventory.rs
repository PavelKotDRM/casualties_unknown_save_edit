//! Inventory tab: items grouped by location (held slot / worn slot) with the
//! container contents, an item details editor and an "add item" catalogue.

use egui::{ProgressBar, RichText, Ui};
use serde_json::{Map, Value, json};

use super::{Names, card, category_icon, colors, quality_color};
use crate::catalog;
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
pub fn location_name(loc: &Location) -> String {
    match loc {
        Location::Slot(s) => format!("Inventory slot {s}"),
        Location::Worn(w) if w.is_empty() => "Unassigned (slot -1, no wear slot)".to_owned(),
        Location::Worn(w) => format!("Worn: {w}"),
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

/// All wear slots known to the catalogue, sorted.
fn wear_slots() -> Vec<&'static str> {
    let mut v: Vec<&str> = catalog::ITEMS
        .iter()
        .map(|d| d.wear_slot)
        .filter(|w| !w.is_empty())
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Locations offered in choosers: existing ones, slots 0–9 and wear slots.
fn location_choices(data: &Value) -> Vec<Location> {
    let mut out: Vec<Location> = model::group_items(data)
        .into_iter()
        .map(|g| g.location)
        .collect();
    for s in 0..10 {
        let l = Location::Slot(s);
        if !out.contains(&l) {
            out.push(l);
        }
    }
    for w in wear_slots() {
        let l = Location::Worn(w.to_owned());
        if !out.contains(&l) {
            out.push(l);
        }
    }
    out
}

fn location_combo(ui: &mut Ui, id: &str, data: &Value, current: &mut Location) -> bool {
    let before = current.clone();
    egui::ComboBox::from_id_salt(id)
        .selected_text(location_name(current))
        .width(240.0)
        .height(400.0)
        .show_ui(ui, |ui| {
            for loc in location_choices(data) {
                let text = format!("{} {}", location_icon(&loc), location_name(&loc));
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
pub fn show(ui: &mut Ui, data: &mut Value, names: Names, state: &mut InventoryState) -> bool {
    let mut changed = false;
    let count = items(data).len();
    if state.selected.is_some_and(|i| i >= count) {
        state.selected = None;
    }
    ui.horizontal(|ui| {
        if ui.button("➕ Add item…").clicked() {
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
        ui.label(format!("{count} items"));
        ui.weak("· The first item of a location is held / worn; the following ones are stored inside it.");
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
                    action = item_list(ui, data, names, state.selected);
                });
        });
        ui.separator();
        ui.vertical(|ui| {
            egui::ScrollArea::vertical()
                .id_salt("inv_details")
                .auto_shrink(false)
                .show(ui, |ui| match state.selected {
                    Some(i) => changed |= details(ui, data, names, state, i),
                    None => {
                        ui.add_space(20.0);
                        ui.weak("Select an item to edit it.");
                    }
                });
        });
    });

    if let Some(a) = action {
        changed |= apply_action(data, state, a);
    }
    if state.add_open {
        changed |= add_window(ui, data, names, state);
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
                    location_name(&group.location)
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
                    if let Some(a) =
                        item_row(ui, &all[i], i, names, selected == Some(i), first, last)
                    {
                        action = Some(a);
                    }
                });
                if first && group.indices.len() > 1 {
                    ui.horizontal(|ui| {
                        ui.add_space(22.0);
                        ui.weak(format!("contains {} item(s):", group.indices.len() - 1));
                    });
                }
            }
        });
        ui.add_space(4.0);
    }
    if all.is_empty() {
        ui.weak("The inventory is empty.");
    }
    action
}

fn item_row(
    ui: &mut Ui,
    item: &Value,
    i: usize,
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
    r.on_hover_ui(|ui| {
        ui.strong(names.item(id));
        ui.weak(format!("id: {id} · items[{i}]"));
        if let Some(d) = names.item_description(id) {
            ui.label(d);
        }
        if def.is_none() {
            ui.colored_label(colors::WARN, "Unknown id — not in the game's item list.");
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
    .on_hover_text("Condition");
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.small_button("🗑").on_hover_text("Delete").clicked() {
            action = Some(RowAction::Delete(i));
        }
        if ui.small_button("⧉").on_hover_text("Duplicate").clicked() {
            action = Some(RowAction::Duplicate(i));
        }
        if ui
            .add_enabled(!last, egui::Button::new("⏷").small())
            .on_hover_text("Move down")
            .clicked()
        {
            action = Some(RowAction::Down(i));
        }
        if ui
            .add_enabled(!first, egui::Button::new("⏶").small())
            .on_hover_text("Move up")
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
        if ui.small_button(star).on_hover_text("Favourite").clicked() {
            action = Some(RowAction::ToggleFavourite(i));
        }
    });
    action
}

fn row_label(ui: &mut Ui, text: &str) {
    ui.add_sized(
        [120.0, ui.spacing().interact_size.y],
        egui::Label::new(text),
    );
}

fn details(
    ui: &mut Ui,
    data: &mut Value,
    names: Names,
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
            "Category: {}{}{}",
            d.category,
            if d.wear_slot.is_empty() {
                String::new()
            } else {
                format!(" · wearable ({})", d.wear_slot)
            },
            if d.capacity > 0.0 {
                format!(" · holds {} ml", super::fmt_num(d.capacity as f64))
            } else {
                String::new()
            },
        ));
    }
    ui.add_space(6.0);

    // Location (moving changes the index).
    let mut loc = Location::of(&item);
    ui.horizontal(|ui| {
        row_label(ui, "Location");
        if location_combo(ui, "item_location", data, &mut loc)
            && let Some(n) = model::move_to_location(data, index, &loc)
        {
            state.selected = Some(n);
            changed = true;
        }
    });
    if changed {
        return true;
    }
    if let (Location::Worn(w), Some(d)) = (&loc, def)
        && !w.is_empty()
        && d.wear_slot != w
        && model::group_items(data)
            .iter()
            .any(|g| g.location == loc && g.indices.first() == Some(&index))
    {
        ui.colored_label(
            colors::WARN,
            format!(
                "⚠ This item's wear slot is \"{}\", not \"{w}\".",
                d.wear_slot
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
        changed |= item_fields(ui, obj, &id);
    });

    // Components
    ui.add_space(8.0);
    ui.label(RichText::new("Components").strong());
    let comps = ensure_components(data, index);
    if let Some(comps) = comps {
        changed |= components_editor(ui, comps, def, names);
    }
    changed
}

fn item_fields(ui: &mut Ui, obj: &mut Map<String, Value>, id: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        row_label(ui, "Item id");
        let mut text = id.to_owned();
        let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(180.0));
        if r.changed() {
            obj.insert("id".into(), Value::String(text));
            changed = true;
        }
        if catalog::item(id).is_none() {
            ui.colored_label(colors::WARN, "unknown id");
        }
    });
    if let Some(slot) = obj.get_mut("condition") {
        ui.horizontal(|ui| {
            row_label(ui, "Condition");
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
            row_label(ui, "Favourited");
            if ui.checkbox(&mut fav, "").changed() {
                json::set_bool(obj, "favourited", fav);
                changed = true;
            }
        });
    }
    if let Some(Value::String(label)) = obj.get_mut("label") {
        ui.horizontal(|ui| {
            row_label(ui, "Label");
            changed |= ui
                .add(
                    egui::TextEdit::singleline(label)
                        .hint_text("custom name")
                        .desired_width(180.0),
                )
                .changed();
        });
    }
    if let Some(slot) = obj.get_mut("swatch") {
        ui.horizontal(|ui| {
            row_label(ui, "Swatch");
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
        ui.weak("No saved component state (the game uses the item's defaults).");
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
                        .on_hover_text("Remove this component (use defaults)")
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
                "BatteryItem" => battery_editor(ui, obj),
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
        && ui.button("💧 Add liquid contents").clicked()
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
        if ui.button("➕ Add liquid").clicked() {
            stack.push(json!({"liquidId": "water", "amount": 0.0}));
            changed = true;
        }
        if capacity > 0.0
            && total > 0.0
            && ui
                .button("Fill to capacity")
                .on_hover_text("Scales all liquids proportionally")
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
        if !stack.is_empty() && ui.button("Empty").clicked() {
            stack.clear();
            changed = true;
        }
    });
    changed
}

fn battery_editor(ui: &mut Ui, obj: &mut Map<String, Value>) -> bool {
    let mut changed = false;
    if let Some(slot) = obj.get_mut("maxCharge") {
        ui.horizontal(|ui| {
            row_label(ui, "Max charge");
            changed |= super::number_editor(ui, slot, 1.0);
        });
    }
    if let Some(Value::String(kind)) = obj.get_mut("batteryType") {
        ui.horizontal(|ui| {
            row_label(ui, "Battery");
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

fn add_window(ui: &mut Ui, data: &mut Value, names: Names, state: &mut InventoryState) -> bool {
    let mut changed = false;
    let mut open = state.add_open;
    let mut close = false;
    egui::Window::new("➕ Add item")
        .open(&mut open)
        .default_size([520.0, 560.0])
        .collapsible(false)
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.add(
                    egui::TextEdit::singleline(&mut state.add_search)
                        .hint_text("Search name or id")
                        .desired_width(200.0),
                );
                let mut cats: Vec<&str> = catalog::ITEMS.iter().map(|d| d.category).collect();
                cats.sort_unstable();
                cats.dedup();
                egui::ComboBox::from_id_salt("add_cat")
                    .selected_text(if state.add_category.is_empty() {
                        "All categories"
                    } else {
                        state.add_category.as_str()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut state.add_category,
                            String::new(),
                            "All categories",
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
                            if !d.wear_slot.is_empty() && state.add_location.is_none() {
                                state.add_location = Some(Location::Worn(d.wear_slot.to_owned()));
                            }
                        }
                        if r.double_clicked() {
                            state.add_id = d.id.to_owned();
                            changed |= do_add(data, state);
                        }
                    }
                    if list.is_empty() {
                        ui.weak("Nothing found.");
                    }
                });
            ui.separator();
            ui.horizontal(|ui| {
                row_label(ui, "Item id");
                ui.add(egui::TextEdit::singleline(&mut state.add_id).desired_width(180.0));
                if !state.add_id.is_empty() {
                    ui.label(names.item(&state.add_id));
                }
            });
            ui.horizontal(|ui| {
                row_label(ui, "Put into");
                let mut loc = state.add_location.clone().unwrap_or(Location::Slot(0));
                location_combo(ui, "add_location", data, &mut loc);
                state.add_location = Some(loc);
                if let Some(d) = catalog::item(&state.add_id).filter(|d| !d.wear_slot.is_empty())
                    && ui.small_button(format!("Wear ({})", d.wear_slot)).clicked()
                {
                    state.add_location = Some(Location::Worn(d.wear_slot.to_owned()));
                }
            });
            if let Some(loc) = &state.add_location {
                let occupied = model::group_items(data).iter().any(|g| &g.location == loc);
                ui.weak(if occupied {
                    "The location is occupied: the item will be stored inside the held / worn item."
                } else {
                    "The location is empty: the item will be held / worn."
                });
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!state.add_id.trim().is_empty(), egui::Button::new("➕ Add"))
                    .clicked()
                {
                    changed |= do_add(data, state);
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });
    state.add_open = open && !close;
    changed
}

fn do_add(data: &mut Value, state: &mut InventoryState) -> bool {
    let loc = state.add_location.clone().unwrap_or(Location::Slot(0));
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
}
