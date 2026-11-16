//! Backup browser: lists the backups of a save, previews what each one
//! contains next to the current save, and restores, opens or deletes them.
//! Several backups can be checked at once, and old ones can be picked by a
//! cleanup rule (keep the newest N, or older than N days).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use egui::{RichText, Ui};

use super::{Names, colors};
use crate::save::{self, BackupInfo, Cleanup, SaveSummary};
use crate::schema;

/// What the user asked the browser to do; carried out by the application.
#[derive(Debug, Clone, PartialEq)]
pub enum BackupAction {
    /// Back up the target save now.
    Create,
    /// Replace the target save with this backup.
    Restore(PathBuf),
    /// Open the backup itself in the editor.
    Open(PathBuf),
    /// Delete these backups.
    Delete(Vec<PathBuf>),
    /// Show this folder in the file manager.
    OpenFolder(PathBuf),
}

/// State of the backup browser window.
pub struct BackupBrowser {
    /// Whether the window is shown.
    pub open: bool,
    /// Save whose backups are listed.
    target: Option<PathBuf>,
    /// Backups of [`Self::target`], newest first.
    list: Vec<BackupInfo>,
    /// Selected backup.
    selected: Option<PathBuf>,
    /// Summary of the selected backup (cached).
    preview: Option<(PathBuf, Result<SaveSummary, String>)>,
    /// Summary of the target file on disk (cached).
    target_summary: Option<Result<SaveSummary, String>>,
    /// Backups checked for a bulk action.
    checked: BTreeSet<PathBuf>,
    /// Backups waiting for a delete confirmation.
    confirm_delete: Option<Vec<PathBuf>>,
    /// Cleanup rule being edited.
    cleanup: Cleanup,
    /// What the list was built from, to notice changes on disk.
    stamp: Vec<(PathBuf, Option<SystemTime>, u64)>,
    /// Area of the window in the last frame.
    rect: Option<egui::Rect>,
}

impl Default for BackupBrowser {
    fn default() -> Self {
        Self {
            open: false,
            target: None,
            list: Vec::new(),
            selected: None,
            preview: None,
            target_summary: None,
            checked: BTreeSet::new(),
            confirm_delete: None,
            cleanup: Cleanup::KeepNewest(10),
            stamp: Vec::new(),
            rect: None,
        }
    }
}

/// Paths, dates and sizes of the backups and of the target itself.
fn stamp_of(target: Option<&Path>, list: &[BackupInfo]) -> Vec<(PathBuf, Option<SystemTime>, u64)> {
    let target = target.map(|t| {
        let meta = std::fs::metadata(t).ok();
        (
            t.to_owned(),
            meta.as_ref().and_then(|m| m.modified().ok()),
            meta.map_or(u64::MAX, |m| m.len()),
        )
    });
    target
        .into_iter()
        .chain(list.iter().map(|b| (b.path.clone(), b.modified, b.size)))
        .collect()
}

impl BackupBrowser {
    /// Opens the window for the backups of `target`.
    pub fn show_for(&mut self, target: Option<PathBuf>) {
        self.open = true;
        self.refresh(target);
    }

    /// Re-reads the backup list only if the backups or the target changed on
    /// disk (or the target is another save), so a periodic refresh does not
    /// disturb the user.
    pub fn refresh_if_changed(&mut self, target: Option<PathBuf>) {
        let list = target
            .as_deref()
            .map(save::list_backups)
            .unwrap_or_default();
        if target != self.target || stamp_of(target.as_deref(), &list) != self.stamp {
            self.refresh(target);
        }
    }

    /// Re-reads the backup list of `target`, keeping the selection, checks
    /// and a pending confirmation for backups that still exist.
    pub fn refresh(&mut self, target: Option<PathBuf>) {
        self.list = target
            .as_deref()
            .map(save::list_backups)
            .unwrap_or_default();
        self.stamp = stamp_of(target.as_deref(), &self.list);
        self.target_summary = target
            .as_deref()
            .filter(|t| t.is_file())
            .map(|t| SaveSummary::load(t).map_err(|e| e.to_string()));
        self.target = target;
        if !self
            .selected
            .as_ref()
            .is_some_and(|s| self.list.iter().any(|b| &b.path == s))
        {
            self.selected = self.list.first().map(|b| b.path.clone());
        }
        let exists = |p: &PathBuf| self.list.iter().any(|b| &b.path == p);
        self.checked.retain(|p| exists(p));
        if self
            .confirm_delete
            .as_ref()
            .is_some_and(|paths| !paths.iter().all(exists))
        {
            self.confirm_delete = None;
        }
        self.preview = None;
    }

    /// Backups checked for a bulk action.
    pub fn checked(&self) -> impl Iterator<Item = &Path> {
        self.checked.iter().map(PathBuf::as_path)
    }

    /// The save whose backups are listed.
    pub fn target(&self) -> Option<&Path> {
        self.target.as_deref()
    }

    /// Number of listed backups.
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// Whether there are no backups.
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Selects a backup by path.
    pub fn select(&mut self, backup: &Path) {
        self.selected = Some(backup.to_owned());
    }

    /// Shows the window. `current` summarises the save open in the editor
    /// when it is the target (so unsaved edits are compared), otherwise the
    /// target file on disk is used.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        names: Names,
        current: Option<SaveSummary>,
    ) -> Option<BackupAction> {
        if !self.open {
            return None;
        }
        if let Some(selected) = self.selected.clone()
            && self.preview.as_ref().is_none_or(|(p, _)| *p != selected)
        {
            let summary = SaveSummary::load(&selected).map_err(|e| e.to_string());
            self.preview = Some((selected, summary));
        }
        let current = current.map(Ok).or_else(|| self.target_summary.clone());
        let mut action = None;
        let mut open = true;
        self.rect = super::bounded_window(
            ctx,
            format!("🗄 {}", names.text("Backups")),
            egui::vec2(640.0, 600.0),
            420.0,
        )
        .open(&mut open)
        .show(ctx, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            self.header(ui, names, &mut action);
            ui.separator();
            if self.list.is_empty() {
                ui.weak(names.text("No backups yet.").to_string());
                return;
            }
            self.bulk_ui(ui, names, &mut action);
            self.list_ui(ui, names);
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("backup_details")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    self.details(ui, names, current.as_ref(), &mut action)
                });
        })
        .map(|shown| shown.response.rect);
        self.open = open;
        action
    }

    fn header(&self, ui: &mut Ui, names: Names, action: &mut Option<BackupAction>) {
        let Some(target) = &self.target else {
            ui.weak(names.text("The save folder was not found.").to_string());
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(names.text("Backups of").to_string());
            ui.label(RichText::new(target.display().to_string()).monospace());
        });
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    target.is_file(),
                    egui::Button::new(format!("➕ {}", names.text("Backup now"))),
                )
                .clicked()
            {
                *action = Some(BackupAction::Create);
            }
            if ui
                .button(format!("📁 {}", names.text("Open folder")))
                .clicked()
            {
                *action = Some(BackupAction::OpenFolder(save::backup_dir(target)));
            }
            ui.weak(
                names
                    .text("{count} backups")
                    .replace("{count}", &self.list.len().to_string()),
            );
        });
    }

    fn list_ui(&mut self, ui: &mut Ui, names: Names) {
        let height = (ui.available_height() * 0.4).clamp(90.0, 260.0);
        egui::ScrollArea::vertical()
            .id_salt("backup_list")
            .max_height(height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let mut clicked = None;
                for backup in &self.list {
                    let selected = self.selected.as_ref() == Some(&backup.path);
                    let text = format!(
                        "{}   ·   {}",
                        modified_text(backup, names),
                        size_text(backup.size)
                    );
                    ui.horizontal(|ui| {
                        let mut checked = self.checked.contains(&backup.path);
                        if ui.checkbox(&mut checked, "").changed() {
                            if checked {
                                self.checked.insert(backup.path.clone());
                            } else {
                                self.checked.remove(&backup.path);
                            }
                        }
                        let response = ui
                            .selectable_label(selected, text)
                            .on_hover_text(file_name(&backup.path));
                        if response.clicked() {
                            // Ctrl/Cmd+click toggles the check like in file managers.
                            if ui.input(|i| i.modifiers.command)
                                && !self.checked.remove(&backup.path)
                            {
                                self.checked.insert(backup.path.clone());
                            }
                            clicked = Some(backup.path.clone());
                        }
                    });
                }
                if let Some(path) = clicked {
                    self.select(&path);
                }
            });
    }

    /// Check-all/none, bulk delete, the cleanup rule and the confirmation.
    fn bulk_ui(&mut self, ui: &mut Ui, names: Names, action: &mut Option<BackupAction>) {
        if let Some(paths) = self.confirm_delete.clone() {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    colors::BAD,
                    names
                        .text("Delete {count} backups permanently?")
                        .replace("{count}", &paths.len().to_string()),
                );
                if ui
                    .button(
                        RichText::new(format!("🗑 {}", names.text("Delete permanently")))
                            .color(colors::BAD),
                    )
                    .clicked()
                {
                    for path in &paths {
                        self.checked.remove(path);
                    }
                    *action = Some(BackupAction::Delete(paths));
                    self.confirm_delete = None;
                }
                if ui.button(names.text("Cancel").to_string()).clicked() {
                    self.confirm_delete = None;
                }
            });
            ui.separator();
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button(names.text("Select all").to_string()).clicked() {
                self.checked = self.list.iter().map(|b| b.path.clone()).collect();
            }
            if ui
                .add_enabled(
                    !self.checked.is_empty(),
                    egui::Button::new(names.text("Select none").to_string()),
                )
                .clicked()
            {
                self.checked.clear();
            }
            let label = names
                .text("Delete selected ({count})")
                .replace("{count}", &self.checked.len().to_string());
            if ui
                .add_enabled(
                    !self.checked.is_empty(),
                    egui::Button::new(format!("🗑 {label}")),
                )
                .clicked()
            {
                // Keep the list order (newest first) for the confirmation.
                self.confirm_delete = Some(
                    self.list
                        .iter()
                        .filter(|b| self.checked.contains(&b.path))
                        .map(|b| b.path.clone())
                        .collect(),
                );
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(names.text("Old backups:").to_string());
            let keep = names.text("Keep the newest").into_owned();
            let older = names.text("Older than (days)").into_owned();
            let mut by_age = matches!(self.cleanup, Cleanup::OlderThanDays(_));
            egui::ComboBox::from_id_salt("backup_cleanup_rule")
                .selected_text(if by_age { &older } else { &keep })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut by_age, false, &keep);
                    ui.selectable_value(&mut by_age, true, &older);
                });
            self.cleanup = match (self.cleanup, by_age) {
                (Cleanup::KeepNewest(_), true) => Cleanup::OlderThanDays(30),
                (Cleanup::OlderThanDays(_), false) => Cleanup::KeepNewest(10),
                (rule, _) => rule,
            };
            match &mut self.cleanup {
                Cleanup::KeepNewest(n) => ui.add(egui::DragValue::new(n).range(0..=1000)),
                Cleanup::OlderThanDays(d) => ui.add(egui::DragValue::new(d).range(0..=3650)),
            };
            let old = save::old_backups(&self.list, self.cleanup, SystemTime::now());
            if ui
                .add_enabled(
                    !old.is_empty(),
                    egui::Button::new(
                        names
                            .text("Select {count}")
                            .replace("{count}", &old.len().to_string()),
                    ),
                )
                .on_hover_text(
                    names
                        .text("Check the backups matched by the rule (then delete them)")
                        .to_string(),
                )
                .clicked()
            {
                self.checked = old.into_iter().collect();
            }
        });
    }
    fn details(
        &mut self,
        ui: &mut Ui,
        names: Names,
        current: Option<&Result<SaveSummary, String>>,
        action: &mut Option<BackupAction>,
    ) {
        let Some((path, preview)) = &self.preview else {
            ui.weak(
                names
                    .text("Select a backup to see what it contains.")
                    .to_string(),
            );
            return;
        };
        let path = path.clone();
        ui.label(RichText::new(file_name(&path)).monospace().strong());
        let backup = match preview {
            Ok(summary) => Some(summary.clone()),
            Err(error) => {
                ui.colored_label(
                    colors::BAD,
                    format!("⚠ {}: {error}", names.text("The backup cannot be read")),
                );
                None
            }
        };
        if let Some(backup) = &backup {
            let current = match current {
                Some(Ok(summary)) => Some(summary),
                Some(Err(error)) => {
                    ui.colored_label(
                        colors::WARN,
                        format!(
                            "⚠ {}: {error}",
                            names.text("The current save cannot be read")
                        ),
                    );
                    None
                }
                None => None,
            };
            comparison(ui, names, backup, current);
        }
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    backup.is_some() && self.target.is_some(),
                    egui::Button::new(format!("⟲ {}", names.text("Restore"))),
                )
                .on_hover_text(
                    names
                        .text("Replace the current save with this backup (the current file is backed up first)")
                        .to_string(),
                )
                .clicked()
            {
                *action = Some(BackupAction::Restore(path.clone()));
            }
            if ui
                .add_enabled(
                    backup.is_some(),
                    egui::Button::new(format!("📂 {}", names.text("Open"))),
                )
                .on_hover_text(names.text("Open the backup itself for viewing/editing").to_string())
                .clicked()
            {
                *action = Some(BackupAction::Open(path.clone()));
            }
            if ui
                .button(format!("🗑 {}", names.text("Delete")))
                .clicked()
            {
                self.confirm_delete = Some(vec![path.clone()]);
            }
        });
    }
}

/// Table of the backup's contents next to the current save; values that
/// differ are highlighted.
fn comparison(ui: &mut Ui, names: Names, backup: &SaveSummary, current: Option<&SaveSummary>) {
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
                .map_or_else(unknown, super::run::format_duration),
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

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

fn modified_text(backup: &BackupInfo, names: Names) -> String {
    backup.modified.map_or_else(
        || names.text("unknown date").into_owned(),
        |t| {
            chrono::DateTime::<chrono::Local>::from(t)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        },
    )
}

fn size_text(bytes: u64) -> String {
    format!("{:.1} KB", bytes as f64 / 1024.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::interface_language::InterfaceLanguage;

    #[test]
    fn browser_previews_the_newest_backup_and_fits_the_screen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(save::SAVE_FILE_NAME);
        let mut data = save::new_game_data();
        std::fs::write(&path, save::encode(&data, save::SaveFormat::Gzip)).unwrap();
        save::create_backup(&path).unwrap();
        data["biome"] = 3.into();

        let mut browser = BackupBrowser::default();
        browser.show_for(Some(path.clone()));
        assert_eq!(browser.len(), 1);

        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 500.0));
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        for _ in 0..4 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                let current = Some(SaveSummary::of(&data));
                browser.show(ui.ctx(), names, current);
            });
            output.textures_delta.clear();
        }
        let (_, preview) = browser.preview.as_ref().expect("newest backup previewed");
        assert_eq!(preview.as_ref().unwrap().biome, Some(0));
        let rect = browser.rect.expect("backup window shown");
        assert!(
            screen.contains_rect(rect),
            "backup window {rect:?} leaves the screen"
        );
    }
    #[test]
    fn checks_survive_a_periodic_refresh_and_cleanup_picks_old_backups() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(save::SAVE_FILE_NAME);
        std::fs::write(&path, b"x").unwrap();
        let first = save::create_backup(&path).unwrap();
        let second = save::create_backup(&path).unwrap();

        let mut browser = BackupBrowser::default();
        browser.show_for(Some(path.clone()));
        browser.checked.insert(first.clone());
        browser.confirm_delete = Some(vec![first.clone()]);

        // Nothing changed: the state is kept as is.
        browser.refresh_if_changed(Some(path.clone()));
        assert_eq!(browser.confirm_delete, Some(vec![first.clone()]));

        // A new backup appears: the list grows, checks stay.
        let third = save::create_backup(&path).unwrap();
        browser.refresh_if_changed(Some(path.clone()));
        assert_eq!(browser.len(), 3);
        assert_eq!(browser.checked().collect::<Vec<_>>(), [first.as_path()]);

        // The checked backup disappears: its check and confirmation go too.
        save::delete_backup(&first).unwrap();
        browser.refresh_if_changed(Some(path.clone()));
        assert_eq!(browser.checked().count(), 0);
        assert_eq!(browser.confirm_delete, None);

        let old = save::old_backups(&browser.list, Cleanup::KeepNewest(1), SystemTime::now());
        assert_eq!(old.len(), 1);
        assert!(old[0] == second || old[0] == third);
    }
}
