use super::*;

impl EditorApp {
    pub(in crate::app) fn status_bar(&mut self, ui: &mut Ui) {
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
            let mut report = None;
            if let Some((text, level, at)) = &self.status {
                let color = match level {
                    Level::Info => ui.visuals().text_color(),
                    Level::Success => colors::GOOD,
                    Level::Error => colors::BAD,
                };
                if *level == Level::Error || at.elapsed() < Duration::from_secs(12) {
                    ui.separator();
                    if *level == Level::Error
                        && ui
                            .small_button("🐞")
                            .on_hover_text(language.text("Report this problem…").to_string())
                            .clicked()
                    {
                        report = Some(text.clone());
                    }
                    ui.colored_label(color, text).on_hover_text(text);
                }
            }
            if let Some(title) = report {
                self.report.open_with(title);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .on_hover_text(language.text("Editor version").to_string());
                ui::about::game_version_label(
                    ui,
                    Names {
                        locale: self.locale.as_ref(),
                        interface_language: language,
                    },
                    self.game_info.as_ref(),
                );
            });
        });
    }
}
