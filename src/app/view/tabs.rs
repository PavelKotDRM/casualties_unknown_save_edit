use super::*;

impl EditorApp {
    pub(super) fn is_uncommitted(&self) -> bool {
        self.save
            .as_ref()
            .is_some_and(|save| self.history.has_uncommitted(&save.data))
    }

    pub(in crate::app) fn tab_bar(&mut self, ui: &mut Ui) {
        let game_ready = self.game_ready();
        let pack = self.selected_interface_pack();
        let language =
            InterfaceLanguage::from_code(Some(&self.config.interface_language), pack.as_deref());
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                let selected = self.tab == tab;
                let text = RichText::new(tab.title(language)).size(15.0);
                if ui
                    .add_enabled(
                        game_ready || tab == Tab::Raw,
                        egui::Button::selectable(selected, text),
                    )
                    .clicked()
                    && !selected
                {
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
                    .add_enabled(self.history.can_redo(), egui::Button::new("↷"))
                    .on_hover_text(language.text("Redo (Ctrl+Y)").to_string())
                    .clicked()
                {
                    self.redo();
                }
                if ui
                    .add_enabled(
                        self.history.can_undo() || self.is_uncommitted(),
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
}
