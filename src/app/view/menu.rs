use super::*;

impl EditorApp {
    pub(in crate::app) fn menu_bar(&mut self, ui: &mut Ui) {
        let loaded = self.save.is_some();
        let game_ready = self.game_ready();
        let pack = self.selected_interface_pack();
        let language =
            InterfaceLanguage::from_code(Some(&self.config.interface_language), pack.as_deref());
        egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(language.text("File").to_string(), |ui| {
                    if ui
                        .add_enabled(game_ready && save::can_create_new_game(), egui::Button::new(format!("🆕 {}", language.text("New save…"))))
                        .on_disabled_hover_text(language.text(if game_ready {
                            "New-run generation is unavailable: start a run in the game and open its save instead"
                        } else {
                            "Game data unavailable. Select the game folder in Game data."
                        }).to_string())
                        .clicked()
                    {
                        self.request(Pending::NewSave);
                        ui.close();
                    }
                    ui.separator();
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
                    let target_exists = self.backup_target().is_some_and(|p| p.is_file());
                    if ui
                        .add_enabled(
                            target_exists,
                            egui::Button::new(format!("🗄 {}", language.text("Create backup now"))),
                        )
                        .clicked()
                    {
                        self.manual_backup();
                        ui.close();
                    }
                    if ui
                        .button(format!("🗄 {}", language.text("Backups…")))
                        .clicked()
                    {
                        self.open_backups();
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
                            self.history.can_undo() || self.is_uncommitted(),
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
                            self.history.can_redo(),
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
                    ui.add_enabled_ui(loaded && game_ready, |ui| {
                        if let Some(data) = self.save.as_mut().map(|s| &mut s.data) {
                            let mut fix = |ui: &mut Ui, label: &str, f: &dyn Fn(&mut Value) -> Result<(), String>| {
                                if ui.button(label).clicked() {
                                    ui::operation_result(ui, f(data));
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
                            ui.weak(format!("{} {}", language.text("Game folder:"), d.display()));
                            ui::about::game_version_label(
                                ui,
                                Names {
                                    locale: self.locale.as_ref(),
                                    interface_language: language,
                                },
                                self.game_info.as_ref(),
                            );
                        }
                        None => {
                            ui.colored_label(
                                colors::WARN,
                                language
                                    .text("Game folder not found — showing ids")
                                    .to_string(),
                            );
                        }
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
                    ui.separator();
                    let mut auto = self.config.auto_refresh;
                    if ui
                        .checkbox(&mut auto, language.text("Refresh data automatically").to_string())
                        .on_hover_text(
                            language
                                .text("Every few seconds re-read the save on disk, unlocks, achievements, backups and the game installation")
                                .to_string(),
                        )
                        .changed()
                    {
                        self.config.auto_refresh = auto;
                        if let Err(error) = self.config.store() {
                            self.error("Could not save editor settings", error);
                        }
                    }
                    if ui
                        .add(
                            egui::Button::new(format!("⟳ {}", language.text("Refresh now")))
                                .shortcut_text("F5"),
                        )
                        .clicked()
                    {
                        self.refresh_external();
                        self.set_status(language.text("Data refreshed").to_string(), Level::Success);
                        ui.close();
                    }
                    ui.separator();
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
                    if ui
                        .add(
                            egui::Button::new(format!("📖 {}", language.text("Game wiki")))
                                .shortcut_text("F1"),
                        )
                        .clicked()
                    {
                        self.wiki.open = true;
                        ui.close();
                    }
                    if ui
                        .button(format!("🏆 {}", language.text("Achievements")))
                        .clicked()
                    {
                        self.achievements_ui.open = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .button(format!("🐞 {}", language.text("Report a problem…")))
                        .clicked()
                    {
                        self.report.open_with("");
                        ui.close();
                    }
                    if ui.button(format!("ℹ {}", language.text("About"))).clicked() {
                        self.about_open = true;
                        ui.close();
                    }
                });
            });
    }
}
