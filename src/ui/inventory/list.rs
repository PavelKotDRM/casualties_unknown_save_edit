//! Grouped inventory rows and per-item actions.

use super::*;

pub(super) fn item_list(
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
