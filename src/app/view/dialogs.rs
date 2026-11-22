use super::*;

impl EditorApp {
    pub(in crate::app) fn dialogs(&mut self, ctx: &egui::Context) {
        let pack = self.selected_interface_pack();
        let language =
            InterfaceLanguage::from_code(Some(&self.config.interface_language), pack.as_deref());
        let achievement_language = self.achievement_language();
        let names = Names {
            locale: self.locale.as_ref(),
            interface_language: language,
        };
        ui::wiki::show(
            ctx,
            &mut self.wiki,
            ui::wiki::WikiData {
                names,
                item_stats: &self.item_stats,
                effects: &self.item_effects,
                crafting: &self.crafting,
                crafting_icons: Some(&self.crafting_icons),
                crafting_icons_error: self.crafting_icons_error.as_deref(),
                drop_sources: &self.drop_sources,
                unlocks: &self.progress,
            },
            &mut ui::spoilers::SpoilerUi::new(
                names,
                &mut self.config.spoilers,
                self.achievements
                    .as_ref()
                    .and_then(|result| result.as_ref().ok()),
                achievement_language,
            ),
        );
        if let Some(pending) = self.pending.clone() {
            let mut choice = None;
            egui::Modal::new(egui::Id::unique("confirm_discard")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.heading(language.text("Unsaved changes").to_string());
                let message = match &pending {
                    Pending::Close => language.text("Save your changes before closing?"),
                    Pending::RestoreBackup { .. } => language
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
                if !matches!(pending, Pending::RestoreBackup { .. })
                    && let Some(s) = self.save.as_mut()
                {
                    s.data = self.clean.clone();
                }
                let close = matches!(pending, Pending::Close);
                self.perform(pending);
                if close {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        let current = self
            .save
            .as_ref()
            .filter(|s| Some(s.path.as_path()) == self.backups.target())
            .map(|s| save::SaveSummary::of(&s.data));
        if let Some(action) = self.backups.show(
            ctx,
            Names {
                locale: self.locale.as_ref(),
                interface_language: language,
            },
            current,
        ) {
            self.backup_action(action);
        }
        ui::about::show(
            ctx,
            &mut self.about_open,
            Names {
                locale: self.locale.as_ref(),
                interface_language: language,
            },
            self.game_info.as_ref(),
            &self.progress,
            self.achievements.as_ref().and_then(|a| a.as_ref().ok()),
            &mut self.config.spoilers,
        );
        let achievement_language = self.achievement_language();
        ui::achievements::show(
            ctx,
            &mut self.achievements_ui,
            Names {
                locale: self.locale.as_ref(),
                interface_language: language,
            },
            achievement_language,
            self.achievements.as_ref(),
        );
        if self.report.open {
            let problems = self.problems();
            if let Some(action) = ui::report::show(
                ctx,
                &mut self.report,
                Names {
                    locale: self.locale.as_ref(),
                    interface_language: language,
                },
                &problems,
                self.crash.is_some(),
            ) {
                self.report_action(action);
            }
        }

        if let Some(mut wizard) = self.new_save_wizard.take() {
            let mut open = true;
            let mut cancel = false;
            let mut create = false;
            let mut can_create = false;
            let mut next = false;
            let mut back = false;
            let names = Names {
                locale: self.locale.as_ref(),
                interface_language: language,
            };
            let unlocks = &mut self.progress;
            let mut spoilers = ui::spoilers::SpoilerUi::new(
                names,
                &mut self.config.spoilers,
                self.achievements
                    .as_ref()
                    .and_then(|result| result.as_ref().ok()),
                achievement_language,
            );
            ui::bounded_window(
                    ctx,
                    format!("🆕 {}", language.text("Create new save")),
                    egui::vec2(760.0, 680.0),
                    540.0,
                )
                .open(&mut open)
                .show(ctx, |ui| {
                        ui.heading(match wizard.step {
                            NewSaveStep::World => language.text("World settings"),
                            NewSaveStep::Character => language.text("Character"),
                        });
                        let footer_height =
                            ui.spacing().interact_size.y + ui.spacing().item_spacing.y * 3.0 + 6.0;
                        egui::ScrollArea::vertical()
                            .id_salt(("new_save_scroll", wizard.step))
                            .auto_shrink(false)
                            .max_height((ui.available_height() - footer_height).max(80.0))
                            .show(ui, |ui| match wizard.step {
                            NewSaveStep::World => {
                                ui.label(
                                    language
                                        .text("Choose a built-in game preset or customize every world setting.")
                                        .to_string(),
                                );
                                ui.horizontal(|ui| {
                                    ui.label(language.text("World preset:").to_string());
                                    let current_name = schema::RUN_PRESETS
                                        .iter()
                                        .find(|preset| preset.id == wizard.preset)
                                        .map_or_else(
                                            || language.text("Custom").into_owned(),
                                            |preset| language.text(preset.name).into_owned(),
                                        );
                                    let previous = wizard.preset.clone();
                                    egui::ComboBox::from_id_salt("new_save_preset")
                                        .selected_text(current_name)
                                        .show_ui(ui, |ui| {
                                            for preset in schema::RUN_PRESETS {
                                                ui.selectable_value(
                                                    &mut wizard.preset,
                                                    preset.id.to_owned(),
                                                    language.text(preset.name),
                                                );
                                            }
                                            ui.selectable_value(
                                                &mut wizard.preset,
                                                "custom".to_owned(),
                                                language.text("Custom"),
                                            );
                                        });
                                    if wizard.preset != previous
                                        && let Some(settings) =
                                            schema::run_settings_for_preset(&wizard.preset)
                                    {
                                        wizard.data["runSettings"] = Value::Array(settings);
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label(language.text("Starting layer:").to_string());
                                    let current = wizard.data["biome"].as_i64().unwrap_or(0);
                                    let mut selected = current;
                                    egui::ComboBox::from_id_salt("new_save_layer")
                                        .selected_text(names.layer(selected))
                                        .show_ui(ui, |ui| {
                                            for (index, _) in schema::LAYERS.iter().enumerate() {
                                                if !spoilers.state.layer_visible(index, unlocks) {
                                                    continue;
                                                }
                                                let index = index as i64;
                                                let reached = unlocks.is_layer_reached(index);
                                                let label = names.layer_if_reached(index, unlocks);
                                                if ui
                                                    .add_enabled(
                                                        reached,
                                                        egui::Button::selectable(selected == index, label),
                                                    )
                                                    .on_disabled_hover_text(
                                                        language
                                                            .text("Not reached in the game yet: descend to this layer in a run first.")
                                                            .to_string(),
                                                    )
                                                    .clicked()
                                                {
                                                    selected = index;
                                                }
                                            }
                                        });
                                    if selected != current {
                                        wizard.data["biome"] = Value::from(selected);
                                    }
                                    if let Some(deepest) = unlocks.deepest_layer() {
                                        ui.weak(
                                            language
                                                .text("Deepest layer reached: {layer}")
                                                .replace("{layer}", &names.layer(deepest)),
                                        );
                                    }
                                    ui::spoilers::layer_total(ui, names, spoilers.state);
                                });
                                ui.separator();
                                let settings_changed = wizard.data["runSettings"]
                                    .as_array_mut()
                                    .is_some_and(|settings| {
                                        ui.push_id("new_save_run_settings", |ui| {
                                            ui::run::run_settings_editor(ui, settings, names)
                                        })
                                        .inner
                                    });
                                if settings_changed {
                                    wizard.preset = "custom".to_owned();
                                }
                                next = true;
                            }
                            NewSaveStep::Character => {
                                ui.label(
                                    language
                                        .text("Choose the character after setting up the world.")
                                        .to_string(),
                                );
                                ui.push_id("new_save_character", |ui| {
                                    ui::character::contents(ui, &mut wizard.data, names, unlocks, true, &mut spoilers);
                                });
                                back = true;
                                can_create = true;
                            }
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(language.text("Cancel")).clicked() {
                                cancel = true;
                            }
                            if back && ui.button(language.text("Back")).clicked() {
                                wizard.step = NewSaveStep::World;
                            }
                            if next && ui.button(language.text("Next")).clicked() {
                                wizard.step = NewSaveStep::Character;
                            }
                            if can_create
                                && ui
                                    .button(format!("💾 {}", language.text("Create save…")))
                                    .clicked()
                            {
                                create = true;
                            }
                        });
                    });
            if cancel {
                open = false;
            }
            if create && open && self.create_new_save(wizard.data.clone()) {
                open = false;
            }
            if open {
                self.new_save_wizard = Some(wizard);
            }
        }
    }
}
