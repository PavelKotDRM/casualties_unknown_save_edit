use super::*;

impl EditorApp {
    pub(in crate::app) fn central(&mut self, ui: &mut Ui) {
        let game_ready = self.game_ready();
        if !game_ready {
            self.tab = Tab::Raw;
        }
        let pack = self.selected_interface_pack();
        let language =
            InterfaceLanguage::from_code(Some(&self.config.interface_language), pack.as_deref());
        if !game_ready {
            ui.colored_label(
                colors::WARN,
                language
                    .text("Game data unavailable. Select the game folder in Game data.")
                    .into_owned(),
            );
        }
        let names = Names {
            locale: self.locale.as_ref(),
            interface_language: language,
        };
        let achievement_language = self.achievement_language();
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
                let mut spoilers = ui::spoilers::SpoilerUi::new(
                    names,
                    &mut self.config.spoilers,
                    self.achievements
                        .as_ref()
                        .and_then(|result| result.as_ref().ok()),
                    achievement_language,
                );
                ui::character::show(ui, data, names, &mut self.progress, &mut spoilers);
            }
            Tab::Health => {
                ui::health::show(ui, data, names);
            }
            Tab::Limbs => {
                ui::limbs::show(ui, data, names, &self.item_stats, &mut self.limbs);
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
