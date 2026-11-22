//! Location labels, icons, and destination selection widget.

use super::{Names, add};
use crate::game_data::ItemStats;
use crate::model::Location;
use crate::model::placement::{HolderRules, location_choices};
use egui::{RichText, Ui};
use serde_json::Value;
use std::collections::HashMap;

/// Human-readable name of a location.
pub fn location_name(loc: &Location, names: Names) -> String {
    let language = names.interface_language;
    match loc {
        Location::Slot(s) => names.hold_slot(*s),
        Location::Worn(w) if w.is_empty() => language
            .text("Unassigned (slot -1, no wear slot)")
            .into_owned(),
        Location::Worn(w) => format!("{}: {}", language.text("Worn"), names.wear_slot(w)),
    }
}
pub(super) fn location_icon(loc: &Location) -> &'static str {
    match loc {
        Location::Slot(s) => usize::try_from(*s)
            .ok()
            .and_then(|i| crate::schema::HOLD_SLOTS.get(i))
            .map_or("❓", |def| def.icon),
        Location::Worn(_) => "👕",
    }
}
#[allow(clippy::too_many_arguments)] // independent pieces of UI state
pub(super) fn location_combo(
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
