//! Health tab: every `body` field grouped like the in-game status screens.

use egui::Ui;
use serde_json::Value;

use super::{card, colors, field_row, other_fields, section_heading};
use crate::{model, schema};

/// Shows the health tab. Returns `true` if the document changed.
pub fn show(ui: &mut Ui, data: &mut Value) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        if ui
            .button("❤ Restore vitals")
            .on_hover_text("Blood, oxygen, heart, consciousness, shock, sickness, temperature…")
            .clicked()
        {
            model::restore_vitals(data);
            changed = true;
        }
        if ui
            .button("🍖 Fill needs")
            .on_hover_text("Food, water, stamina, energy; clean and dry")
            .clicked()
        {
            model::fill_needs(data);
            changed = true;
        }
    });
    ui.separator();
    let Some(body) = data.get_mut("body").and_then(Value::as_object_mut) else {
        ui.colored_label(colors::BAD, "This save has no `body` object.");
        return changed;
    };
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            let two_columns = ui.available_width() > 1000.0;
            let bar = if two_columns { 180.0 } else { 260.0 };
            let mut render_group = |ui: &mut Ui, group: &schema::FieldGroup| {
                section_heading(ui, group.icon, group.title);
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for spec in group.fields {
                        changed |= field_row(ui, spec, body, bar);
                    }
                });
                ui.add_space(6.0);
            };
            if two_columns {
                ui.columns(2, |cols| {
                    for (i, group) in schema::BODY_GROUPS.iter().enumerate() {
                        render_group(&mut cols[i % 2], group);
                    }
                });
            } else {
                for group in schema::BODY_GROUPS {
                    render_group(ui, group);
                }
            }
            let known = |k: &str| schema::body_field(k).is_some() || k == "skills";
            changed |= other_fields(ui, "body_other", body, &known);
        });
    changed
}
