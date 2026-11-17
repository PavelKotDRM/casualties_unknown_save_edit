//! Save summary comparison shown in the backup browser.

use super::*;
use crate::schema;

/// Table of the backup's contents next to the current save; values that
/// differ are highlighted.
pub(super) fn comparison(
    ui: &mut Ui,
    names: Names,
    backup: &SaveSummary,
    current: Option<&SaveSummary>,
) {
    let rows = summary_rows(names, backup);
    let current_rows = current.map(|c| summary_rows(names, c));
    egui::Grid::new("backup_comparison")
        .striped(true)
        .num_columns(3)
        .spacing([16.0, 4.0])
        .show(ui, |ui| {
            ui.label("");
            ui.label(RichText::new(names.text("Backup").to_string()).strong());
            ui.label(RichText::new(names.text("Current save").to_string()).strong());
            ui.end_row();
            for (i, (label, value)) in rows.iter().enumerate() {
                let other = current_rows.as_ref().and_then(|rows| rows.get(i));
                let differs = other.is_some_and(|(_, other)| other != value);
                ui.label(label);
                let text = RichText::new(value);
                ui.label(if differs {
                    text.color(colors::ACCENT).strong()
                } else {
                    text
                });
                ui.label(other.map_or_else(|| "—".to_owned(), |(_, v)| v.clone()));
                ui.end_row();
            }
        });
}

/// `(label, value)` rows describing a save.
fn summary_rows(names: Names, summary: &SaveSummary) -> Vec<(String, String)> {
    let unknown = || "—".to_owned();
    let mut rows = vec![
        (
            names.text("Character").into_owned(),
            summary
                .character
                .map_or_else(unknown, |c| names.character(c)),
        ),
        (
            names.text("Layer").into_owned(),
            summary.biome.map_or_else(unknown, |b| names.layer(b)),
        ),
        (
            names.text("Run time").into_owned(),
            summary
                .run_time
                .map_or_else(unknown, super::super::run::format_duration),
        ),
        (
            names.text("Distance").into_owned(),
            summary
                .traveled
                .map_or_else(unknown, |d| format!("{d} {}", names.text("m"))),
        ),
        (names.text("Items").into_owned(), summary.items.to_string()),
        (
            names.text("Blood").into_owned(),
            summary.blood.map_or_else(unknown, |b| format!("{b:.0}%")),
        ),
    ];
    for (def, level) in schema::SKILLS.iter().zip(&summary.skills) {
        rows.push((names.skill(def), level.to_string()));
    }
    if summary.cheated {
        rows.push((
            names.text("Cheated save").into_owned(),
            names.text("yes").into_owned(),
        ));
    }
    rows
}
