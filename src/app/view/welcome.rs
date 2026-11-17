use super::*;

impl EditorApp {
    pub(in crate::app) fn welcome(&mut self, ui: &mut Ui) {
        let game_ready = self.game_ready();
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
                ui.weak(format!(
                    "{} v{}",
                    language.text("Editor"),
                    env!("CARGO_PKG_VERSION")
                ));
                ui::about::game_version_label(
                    ui,
                    Names {
                        locale: self.locale.as_ref(),
                        interface_language: language,
                    },
                    self.game_info.as_ref(),
                );
                ui.add_space(10.0);
                if !game_ready {
                    ui.colored_label(colors::WARN, language.text("Game data unavailable. Select the game folder in Game data.").to_string());
                }
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
                if ui.add_enabled(game_ready && save::can_create_new_game(), egui::Button::new(format!("🆕 {}", language.text("Create new save…")))).on_disabled_hover_text(language.text(if game_ready {
                    "New-run generation is unavailable: start a run in the game and open its save instead"
                } else {
                    "Game data unavailable. Select the game folder in Game data."
                }).to_string()).clicked() {
                    self.request(Pending::NewSave);
                }
                ui.add_space(6.0);
                if ui.button(format!("📖 {}", language.text("Game wiki"))).clicked() {
                    self.wiki.open = true;
                }
                ui.add_space(6.0);
                if ui.button(format!("🗄 {}", language.text("Backups…"))).clicked() {
                    self.open_backups();
                }
                ui.add_space(6.0);
                if ui.button(format!("🏆 {}", language.text("Achievements"))).clicked() {
                    self.achievements_ui.open = true;
                }
                ui.add_space(6.0);
                ui.weak(language.text("A timestamped backup is created in SaveEditorBackups every time a save is opened.").to_string());
            });
    }
}
