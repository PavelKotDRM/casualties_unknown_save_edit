//! Application menus and windows.

use super::*;

impl EditorApp {
    pub(super) fn menu_bar(&mut self, ui: &mut Ui) {
        let loaded = self.save.is_some();
        let pack = self.selected_interface_pack();
        let language =
            InterfaceLanguage::from_code(Some(&self.config.interface_language), pack.as_deref());
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(language.text("File").to_string(), |ui| {
                if ui
                    .button(format!("📂 {}", language.text("Open game save")))
                    .clicked()
                {
                    if let Some(p) = save::default_save_path() {
                        self.request(Pending::Open(p));
                    }
                    ui.close();
                }
                if ui
                    .add(
                        egui::Button::new(format!("📂 {}", language.text("Open…")))
                            .shortcut_text("Ctrl+O"),
                    )
                    .clicked()
                {
                    self.request(Pending::OpenDialog);
                    ui.close();
                }
                if ui
                    .add_enabled(
                        loaded,
                        egui::Button::new(format!("⟲ {}", language.text("Reload from disk"))),
                    )
                    .clicked()
                {
                    self.request(Pending::Reload);
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        loaded,
                        egui::Button::new(format!("💾 {}", language.text("Save")))
                            .shortcut_text("Ctrl+S"),
                    )
                    .clicked()
                {
                    self.save();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        loaded,
                        egui::Button::new(format!("💾 {}", language.text("Save as…"))),
                    )
                    .clicked()
                {
                    self.save_as();
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        loaded,
                        egui::Button::new(format!("📤 {}", language.text("Export readable JSON…"))),
                    )
                    .clicked()
                {
                    self.export_json();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        loaded,
                        egui::Button::new(format!("📥 {}", language.text("Import JSON…"))),
                    )
                    .clicked()
                {
                    self.import_json();
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        loaded,
                        egui::Button::new(format!("🗄 {}", language.text("Create backup now"))),
                    )
                    .clicked()
                {
                    self.manual_backup();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        loaded,
                        egui::Button::new(format!("🗄 {}", language.text("Backups…"))),
                    )
                    .clicked()
                {
                    self.backups_open = true;
                    if let Some(s) = &self.save {
                        self.backups = save::list_backups(&s.path);
                    }
                    ui.close();
                }
                if ui
                    .button(format!("📁 {}", language.text("Open save folder")))
                    .clicked()
                {
                    if let Some(d) = self.current_dir() {
                        Self::open_folder(&d);
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button(format!("🚪 {}", language.text("Exit"))).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    ui.close();
                }
            });
            ui.menu_button(language.text("Edit").to_string(), |ui| {
                if ui
                    .add_enabled(
                        !self.undo.is_empty() || self.is_uncommitted(),
                        egui::Button::new(format!("↶ {}", language.text("Undo")))
                            .shortcut_text("Ctrl+Z"),
                    )
                    .clicked()
                {
                    self.undo();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        !self.redo.is_empty(),
                        egui::Button::new(format!("↷ {}", language.text("Redo")))
                            .shortcut_text("Ctrl+Y"),
                    )
                    .clicked()
                {
                    self.redo();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.is_dirty(),
                        egui::Button::new(format!("⟲ {}", language.text("Revert all changes"))),
                    )
                    .clicked()
                {
                    self.commit();
                    if let Some(s) = self.save.as_mut() {
                        s.data = self.clean.clone();
                    }
                    self.commit();
                    ui.close();
                }
                ui.separator();
                ui.add_enabled_ui(loaded, |ui| {
                    if let Some(data) = self.save.as_mut().map(|s| &mut s.data) {
                        let mut fix = |ui: &mut Ui, label: &str, f: &dyn Fn(&mut Value)| {
                            if ui.button(label).clicked() {
                                f(data);
                                ui.close();
                            }
                        };
                        fix(
                            ui,
                            &format!("✚ {}", language.text("Heal all limbs")),
                            &|d| crate::model::heal_limbs(d, false),
                        );
                        fix(
                            ui,
                            &format!("✚ {}", language.text("Heal + regrow limbs")),
                            &|d| crate::model::heal_limbs(d, true),
                        );
                        fix(
                            ui,
                            &format!("❤ {}", language.text("Restore vitals")),
                            &crate::model::restore_vitals,
                        );
                        fix(
                            ui,
                            &format!("🍖 {}", language.text("Fill needs")),
                            &crate::model::fill_needs,
                        );
                    }
                });
            });
            ui.menu_button(language.text("View").to_string(), |ui| {
                for tab in Tab::ALL {
                    if ui
                        .selectable_label(self.tab == tab, tab.title(language))
                        .clicked()
                    {
                        self.tab = tab;
                        ui.close();
                    }
                }
                ui.separator();
                egui::widgets::global_theme_preference_buttons(ui);
            });
            ui.menu_button(language.text("Game data").to_string(), |ui| {
                match &self.config.game_dir {
                    Some(d) => {
                        ui.weak(format!("{} {}", language.text("Game folder:"), d.display()))
                    }
                    None => ui.colored_label(
                        colors::WARN,
                        language
                            .text("Game folder not found — showing ids")
                            .to_string(),
                    ),
                };
                ui.separator();
                ui.label(language.text("Language of names:").to_string());
                let langs = self.languages.clone();
                let current = self.config.language.clone();
                for l in langs {
                    if ui
                        .selectable_label(current.as_deref() == Some(&l.code), &l.code)
                        .clicked()
                    {
                        self.set_language(Some(l));
                        ui.close();
                    }
                }
                ui.separator();
                ui.label(language.text("Interface language:").to_string());
                let mut choices = vec![
                    ("en".to_owned(), language.text("English").into_owned()),
                    ("ru".to_owned(), language.text("Russian").into_owned()),
                ];
                choices.extend(
                    self.interface_packs
                        .iter()
                        .map(|pack| (pack.code.clone(), pack.name.clone())),
                );
                for (code, label) in choices {
                    if ui
                        .selectable_label(self.config.interface_language == code, label)
                        .clicked()
                    {
                        self.set_interface_language(&code);
                        ui.close();
                    }
                }
                if ui
                    .button(language.text("Reload interface languages").to_string())
                    .clicked()
                {
                    self.reload_interface_packs();
                    ui.close();
                }
                ui.separator();
                if ui
                    .button(format!("📁 {}", language.text("Set game folder…")))
                    .clicked()
                {
                    if let Some(dir) = rfd::FileDialog::new()
                        .set_title(
                            language
                                .text("Casualties Unknown installation folder")
                                .to_string(),
                        )
                        .pick_folder()
                    {
                        if locale::is_game_dir(&dir) {
                            match self.set_game_dir(Some(dir)) {
                                Ok(()) => self.set_status(
                                    language.text("Game data loaded").to_string(),
                                    Level::Success,
                                ),
                                Err(error) => self.error("Could not sync item stats", error),
                            }
                        } else {
                            self.set_status(
                                language
                                    .text(
                                        "That folder does not contain CasualtiesUnknown_Data\\Lang",
                                    )
                                    .to_string(),
                                Level::Error,
                            );
                        }
                    }
                    ui.close();
                }
                if ui
                    .button(format!("🔍 {}", language.text("Detect automatically")))
                    .clicked()
                {
                    let dir = locale::detect_game_dir(save::default_save_dir().as_deref());
                    let found = dir.is_some();
                    match self.set_game_dir(dir) {
                        Ok(()) => self.set_status(
                            if found {
                                language.text("Game folder detected").to_string()
                            } else {
                                language.text("Game folder not found").to_string()
                            },
                            if found { Level::Success } else { Level::Error },
                        ),
                        Err(error) => self.error("Could not sync item stats", error),
                    }
                    ui.close();
                }
            });
            ui.menu_button(language.text("Help").to_string(), |ui| {
                if ui.button(format!("ℹ {}", language.text("About"))).clicked() {
                    self.about_open = true;
                    ui.close();
                }
            });
        });
    }

    fn is_uncommitted(&self) -> bool {
        self.save.as_ref().is_some_and(|s| s.data != self.committed)
    }

    pub(super) fn tab_bar(&mut self, ui: &mut Ui) {
        let pack = self.selected_interface_pack();
        let language =
            InterfaceLanguage::from_code(Some(&self.config.interface_language), pack.as_deref());
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                let selected = self.tab == tab;
                let text = RichText::new(tab.title(language)).size(15.0);
                if ui.add(egui::Button::selectable(selected, text)).clicked() && !selected {
                    self.tab = tab;
                    if tab == Tab::Raw && !self.raw.text_dirty {
                        self.raw.text.clear();
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let dirty = self.is_dirty();
                if ui
                    .add_enabled(
                        dirty,
                        egui::Button::new(
                            RichText::new(format!("💾 {}", language.text("Save"))).strong(),
                        ),
                    )
                    .on_hover_text(language.text("Write the save (Ctrl+S)").to_string())
                    .clicked()
                {
                    self.save();
                }
                if ui
                    .add_enabled(!self.redo.is_empty(), egui::Button::new("↷"))
                    .on_hover_text(language.text("Redo (Ctrl+Y)").to_string())
                    .clicked()
                {
                    self.redo();
                }
                if ui
                    .add_enabled(
                        !self.undo.is_empty() || self.is_uncommitted(),
                        egui::Button::new("↶"),
                    )
                    .on_hover_text(language.text("Undo (Ctrl+Z)").to_string())
                    .clicked()
                {
                    self.undo();
                }
            });
        });
    }

    pub(super) fn status_bar(&mut self, ui: &mut Ui) {
        let pack = self.selected_interface_pack();
        let language =
            InterfaceLanguage::from_code(Some(&self.config.interface_language), pack.as_deref());
        ui.horizontal(|ui| {
            if let Some(file) = &self.save {
                let dirty = file.data != self.clean;
                if dirty {
                    ui.colored_label(colors::WARN, format!("● {}", language.text("modified")));
                } else {
                    ui.colored_label(colors::GOOD, format!("✔ {}", language.text("saved")));
                }
                ui.separator();
                ui.label(RichText::new(file.path.display().to_string()).monospace())
                    .on_hover_text(language.text("Current file").to_string());
                ui.weak(match file.format {
                    SaveFormat::Gzip => "gzip",
                    SaveFormat::PlainJson => "plain JSON",
                });
                if let Some(b) = &self.last_backup {
                    ui.separator();
                    ui.weak(language.text("backup:").to_string())
                        .on_hover_text(b.display().to_string());
                    ui.weak(
                        b.file_name()
                            .map_or(String::new(), |n| n.to_string_lossy().into_owned()),
                    );
                }
            }
            if let Some((text, level, at)) = &self.status {
                let color = match level {
                    Level::Info => ui.visuals().text_color(),
                    Level::Success => colors::GOOD,
                    Level::Error => colors::BAD,
                };
                if *level == Level::Error || at.elapsed() < Duration::from_secs(12) {
                    ui.separator();
                    ui.colored_label(color, text).on_hover_text(text);
                }
            }
        });
    }

    pub(super) fn welcome(&mut self, ui: &mut Ui) {
        let pack = self.selected_interface_pack();
        let language =
            InterfaceLanguage::from_code(Some(&self.config.interface_language), pack.as_deref());
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.2);
            ui.label(
                RichText::new(language.text("Casualties Unknown — Save Editor").to_string())
                    .size(28.0)
                    .strong(),
            );
            ui.add_space(10.0);
            match save::default_save_path() {
                Some(p) if p.exists() => {
                    ui.label(format!("{} {}", language.text("Save found:"), p.display()));
                    if ui.button(RichText::new(format!("📂 {}", language.text("Open game save"))).size(18.0)).clicked() {
                        self.open(&p);
                    }
                }
                Some(p) => {
                    ui.label(format!("{} {}", language.text("No save at"), p.display()));
                    ui.weak(language.text("The game writes save.sv on \"Save & Quit\" and deletes it when the run is loaded again.").to_string());
                }
                None => {
                    ui.label(language.text("Cannot determine the save folder.").to_string());
                }
            }
            ui.add_space(6.0);
            if ui.button(format!("📂 {}", language.text("Open another file…"))).clicked() {
                self.open_dialog();
            }
            ui.add_space(6.0);
            ui.weak(language.text("A timestamped backup is created in SaveEditorBackups every time a save is opened.").to_string());
        });
    }

    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        let pack = self.selected_interface_pack();
        let language =
            InterfaceLanguage::from_code(Some(&self.config.interface_language), pack.as_deref());
        if let Some(pending) = self.pending.clone() {
            let mut choice = None;
            egui::Modal::new(egui::Id::unique("confirm_discard")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.heading(language.text("Unsaved changes").to_string());
                let message = match &pending {
                    Pending::Close => language.text("Save your changes before closing?"),
                    Pending::RestoreBackup(_) => language
                        .text("Restoring a backup discards your unsaved changes. Save them first?"),
                    _ => language.text("Save your changes before continuing?"),
                };
                ui.label(message.to_string());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(format!("💾 {}", language.text("Save"))).clicked() {
                        choice = Some(true);
                    }
                    if ui
                        .button(
                            RichText::new(language.text("Discard").to_string()).color(colors::BAD),
                        )
                        .clicked()
                    {
                        choice = Some(false);
                    }
                    if ui.button(language.text("Cancel").to_string()).clicked() {
                        self.pending = None;
                    }
                });
            });
            if let Some(save_first) = choice {
                self.pending = None;
                if save_first {
                    self.save();
                    if self.is_dirty() {
                        return; // saving failed; keep the editor open
                    }
                }
                if let Some(s) = self.save.as_mut() {
                    s.data = self.clean.clone();
                }
                let close = matches!(pending, Pending::Close);
                self.perform(pending);
                if close {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        if self.backups_open {
            let mut open = true;
            let mut restore = None;
            let mut open_backup = None;
            egui::Window::new(format!("🗄 {}", language.text("Backups")))
                .open(&mut open)
                .default_size([560.0, 360.0])
                .show(ctx, |ui| {
                    let Some(path) = self.save.as_ref().map(|s| s.path.clone()) else { return };
                    let dir = save::backup_dir(&path);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(dir.display().to_string()).monospace());
                        if ui.button(format!("📁 {}", language.text("Open folder"))).clicked() {
                            let _ = std::fs::create_dir_all(&dir);
                            Self::open_folder(&dir);
                        }
                        if ui.button(format!("➕ {}", language.text("Backup now"))).clicked() {
                            self.manual_backup();
                        }
                    });
                    ui.separator();
                    if self.backups.is_empty() {
                        ui.weak(language.text("No backups yet.").to_string());
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("backups").striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                            for b in &self.backups {
                                let name = b.path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
                                ui.label(RichText::new(name).monospace());
                                let when = b.modified.map_or(String::from("?"), |t| {
                                    chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M:%S").to_string()
                                });
                                ui.label(when);
                                ui.label(format!("{:.1} KB", b.size as f64 / 1024.0));
                                if ui.button(format!("⟲ {}", language.text("Restore"))).on_hover_text(language.text("Replace the current save with this backup (the current file is backed up first)").to_string()).clicked() {
                                    restore = Some(b.path.clone());
                                }
                                if ui.button(format!("📂 {}", language.text("Open"))).on_hover_text(language.text("Open the backup itself for viewing/editing").to_string()).clicked() {
                                    open_backup = Some(b.path.clone());
                                }
                                ui.end_row();
                            }
                        });
                    });
                });
            self.backups_open = open;
            if let Some(p) = restore {
                self.request(Pending::RestoreBackup(p));
            }
            if let Some(p) = open_backup {
                self.request(Pending::Open(p));
            }
        }

        if self.about_open {
            egui::Window::new(language.text("About").to_string())
                .open(&mut self.about_open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading(language.text("Casualties Unknown Save Editor").to_string());
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.add_space(6.0);
                    ui.label(language.text("Edits save.sv (gzip-compressed JSON). Unknown fields are preserved as-is.").to_string());
                    ui.label(language.text("Item, limb and setting names are read from the game's own language files.").to_string());
                    ui.label(language.text("Built with egui.").to_string());
                });
        }
    }

    pub(super) fn central(&mut self, ui: &mut Ui) {
        let pack = self.selected_interface_pack();
        let language =
            InterfaceLanguage::from_code(Some(&self.config.interface_language), pack.as_deref());
        let names = Names {
            locale: self.locale.as_ref(),
            interface_language: language,
        };
        let Some(file) = self.save.as_mut() else {
            return;
        };
        let data = &mut file.data;
        match self.tab {
            Tab::Overview => {
                let r = ui::overview::show(ui, data, names);
                if let Some(limb) = r.open_limb {
                    self.limbs.selected = limb;
                    self.tab = Tab::Limbs;
                }
            }
            Tab::Character => {
                ui::character::show(ui, data, names);
            }
            Tab::Health => {
                ui::health::show(ui, data, names);
            }
            Tab::Limbs => {
                ui::limbs::show(ui, data, names, &mut self.limbs);
            }
            Tab::Inventory => {
                ui::inventory::show(ui, data, names, &self.item_stats, &mut self.inventory);
            }
            Tab::Run => {
                ui::run::show(ui, data, names);
            }
            Tab::Raw => {
                ui::raw::show(ui, data, &mut self.raw, language);
            }
        }
    }
}
