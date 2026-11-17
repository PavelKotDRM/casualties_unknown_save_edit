//! Explicit, persistent consent to reveal progression spoilers.

use std::collections::{BTreeMap, BTreeSet};

use egui::Ui;
use serde_json::{Map, Value, json};

use super::Names;
use crate::{achievements::Achievements, player_prefs::GameProgress, schema};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Spoilers {
    pub layer_total: bool,
    pub ending_total: bool,
    ending_hints: BTreeMap<String, BTreeSet<String>>,
}

impl Spoilers {
    pub fn from_json(value: Value) -> Result<Self, serde_json::Error> {
        let fields: Map<String, Value> = serde_json::from_value(value)?;
        let flag = |key| -> Result<bool, serde_json::Error> {
            Ok(fields
                .get(key)
                .cloned()
                .map(serde_json::from_value)
                .transpose()?
                .unwrap_or(false))
        };
        Ok(Self {
            layer_total: flag("layerTotal")?,
            ending_total: flag("endingTotal")?,
            ending_hints: fields
                .get("endingHints")
                .cloned()
                .map(serde_json::from_value)
                .transpose()?
                .unwrap_or_default(),
        })
    }

    pub fn to_json(&self) -> Value {
        json!({
            "layerTotal": self.layer_total,
            "endingTotal": self.ending_total,
            "endingHints": self.ending_hints,
        })
    }

    pub fn layer_visible(&self, index: usize, progress: &GameProgress) -> bool {
        self.layer_total || progress.is_layer_reached(index as i64)
    }

    pub fn ending_visible(&self, index: usize, progress: &GameProgress) -> bool {
        self.ending_total
            || progress.is_ending_discovered(index)
            || self.revealed(index).is_some_and(|hints| !hints.is_empty())
    }

    fn revealed(&self, index: usize) -> Option<&BTreeSet<String>> {
        schema::ENDINGS
            .get(index)
            .and_then(|ending| self.ending_hints.get(ending.pref_key))
    }

    fn reveal_next(&mut self, index: usize, hints: &EndingHints) {
        if let Some(hint) = hints.next(index, self)
            && let Some(ending) = schema::ENDINGS.get(index)
        {
            self.ending_hints
                .entry(ending.pref_key.to_owned())
                .or_default()
                .insert(hint.key.to_owned());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    key: &'static str,
    pub label: &'static str,
    pub text: String,
}

/// Only text supplied by the installed game or Steam's local cache is used.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EndingHints(Vec<Vec<Hint>>);

impl EndingHints {
    pub fn new(names: Names, achievements: Option<&Achievements>, language: &str) -> Self {
        Self(
            schema::ENDINGS
                .iter()
                .map(|ending| {
                    let mut hints = Vec::new();
                    if let Some(achievement) = achievements.and_then(|achievements| {
                        achievements.list.iter().find(|achievement| {
                            achievement
                                .api_name
                                .eq_ignore_ascii_case(ending.achievement)
                        })
                    }) {
                        let description = achievement.description(language).trim();
                        if !description.is_empty() {
                            hints.push(Hint {
                                key: "description",
                                label: "Hint",
                                text: super::strip_rich_text(description),
                            });
                        }
                    }
                    hints.push(Hint {
                        key: "name",
                        label: "Ending name",
                        text: names.text(ending.name).into_owned(),
                    });
                    let status = names
                        .locale
                        .and_then(|locale| locale.other(ending.status_key))
                        .unwrap_or(ending.status);
                    if !status.trim().is_empty() {
                        hints.push(Hint {
                            key: "status",
                            label: "Ending status",
                            text: super::strip_rich_text(status),
                        });
                    }
                    if !ending.how.trim().is_empty() {
                        hints.push(Hint {
                            key: "how",
                            label: "How to reach it",
                            text: names.text(ending.how).into_owned(),
                        });
                    }
                    hints
                })
                .collect(),
        )
    }

    fn steps(&self, index: usize) -> &[Hint] {
        self.0.get(index).map(Vec::as_slice).unwrap_or_default()
    }

    pub fn visible(&self, index: usize, state: &Spoilers) -> Vec<&Hint> {
        self.steps(index)
            .iter()
            .filter(|hint| {
                state
                    .revealed(index)
                    .is_some_and(|revealed| revealed.contains(hint.key))
            })
            .collect()
    }

    fn next(&self, index: usize, state: &Spoilers) -> Option<&Hint> {
        self.steps(index).iter().find(|hint| {
            !state
                .revealed(index)
                .is_some_and(|revealed| revealed.contains(hint.key))
        })
    }

    pub fn title(
        &self,
        index: usize,
        names: Names,
        progress: &GameProgress,
        state: &Spoilers,
    ) -> String {
        if progress.is_ending_discovered(index) {
            return names.ending(index);
        }
        if let Some(hint) = self
            .visible(index, state)
            .iter()
            .find(|hint| hint.key == "name")
        {
            return hint.text.clone();
        }
        if state.ending_total {
            names.ending_if_discovered(index, progress)
        } else {
            names.text("Undiscovered ending").into_owned()
        }
    }
}

pub struct SpoilerUi<'a> {
    pub state: &'a mut Spoilers,
    pub hints: EndingHints,
}

impl<'a> SpoilerUi<'a> {
    pub fn new(
        names: Names,
        state: &'a mut Spoilers,
        achievements: Option<&Achievements>,
        language: &str,
    ) -> Self {
        Self {
            state,
            hints: EndingHints::new(names, achievements, language),
        }
    }
}

pub fn layer_total(ui: &mut Ui, names: Names, state: &mut Spoilers) {
    if state.layer_total {
        ui.label(
            names
                .text("Total layers: {total}")
                .replace("{total}", &schema::LAYERS.len().to_string()),
        );
        if ui.small_button(names.text("Hide layer total")).clicked() {
            state.layer_total = false;
        }
    } else if ui.small_button(names.text("Reveal total layers")).clicked() {
        state.layer_total = true;
    }
}

pub fn ending_total(ui: &mut Ui, names: Names, state: &mut Spoilers) {
    if state.ending_total {
        ui.label(
            names
                .text("Total endings: {total}")
                .replace("{total}", &schema::ENDINGS.len().to_string()),
        );
        if ui.small_button(names.text("Hide ending total")).clicked() {
            state.ending_total = false;
        }
    } else if ui
        .small_button(names.text("Reveal total endings"))
        .clicked()
    {
        state.ending_total = true;
    }
}

pub fn hint_button(
    ui: &mut Ui,
    names: Names,
    index: usize,
    state: &mut Spoilers,
    hints: &EndingHints,
) {
    if hints.next(index, state).is_some()
        && ui
            .small_button(names.text("Reveal next hint"))
            .on_hover_text(names.text(
                "Reveal one more available detail from the game or Steam. This choice is remembered.",
            ))
            .clicked()
    {
        state.reveal_next(index, hints);
    }
}

pub fn next_ending(
    ui: &mut Ui,
    names: Names,
    progress: &GameProgress,
    state: &mut Spoilers,
    hints: &EndingHints,
) {
    let next = (0..schema::ENDINGS.len())
        .find(|&index| !progress.is_ending_discovered(index) && state.revealed(index).is_none());
    // Its absence must not reveal that every ending has already been found.
    let response = ui.small_button(names.text("Reveal a hint for another ending"));
    let message_id = response.id.with("no_more_endings");
    if response.clicked() {
        ui.data_mut(|data| data.insert_temp(message_id, next.is_none()));
        if let Some(index) = next {
            state.reveal_next(index, hints);
        }
    }
    if next.is_none()
        && ui
            .data(|data| data.get_temp::<bool>(message_id))
            .unwrap_or(false)
    {
        ui.weak(names.text("No other hidden endings are available."));
    }
    if !state.ending_hints.is_empty() && ui.small_button(names.text("Hide ending hints")).clicked()
    {
        state.ending_hints.clear();
    }
}

pub fn ending_picker(
    ui: &mut Ui,
    names: Names,
    progress: &GameProgress,
    state: &mut Spoilers,
    hints: &EndingHints,
    selected: &mut Option<usize>,
) {
    let candidates: Vec<_> = (0..schema::ENDINGS.len())
        .filter(|&index| {
            state.ending_visible(index, progress) && !progress.is_ending_discovered(index)
        })
        .collect();
    if !selected.is_some_and(|index| candidates.contains(&index)) {
        *selected = candidates.first().copied();
    }
    if let Some(index) = *selected {
        egui::ComboBox::from_id_salt("ending_hint")
            .selected_text(hints.title(index, names, progress, state))
            .show_ui(ui, |ui| {
                for index in candidates {
                    ui.selectable_value(
                        selected,
                        Some(index),
                        hints.title(index, names, progress, state),
                    );
                }
            });
        hint_button(ui, names, index, state, hints);
    }
    next_ending(ui, names, progress, state, hints);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::achievements::Achievement;
    use crate::game_data::{metadata::Metadata, runtime::scoped_for_test};
    use crate::ui::InterfaceLanguage;

    fn ending(key: &'static str) -> schema::EndingDef {
        schema::EndingDef {
            pref_key: key,
            achievement: key,
            name: "Secret destination",
            status_key: "",
            status: "Classified outcome",
            how: "Detailed route",
        }
    }

    fn names() -> Names<'static> {
        Names {
            locale: None,
            interface_language: InterfaceLanguage::English,
        }
    }

    fn achievements() -> Achievements {
        Achievements {
            list: vec![Achievement {
                api_name: "first".into(),
                stat: "1".into(),
                bit: 0,
                names: vec![],
                descriptions: vec![("english".into(), "Look for <b>a way out</b>.".into())],
                hidden: true,
                unlocked: false,
                unlocked_at: None,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn each_request_reveals_only_one_detail_for_one_ending() {
        let _metadata = scoped_for_test(Metadata {
            endings: vec![ending("first"), ending("second")],
            ..Default::default()
        });
        let progress = GameProgress::from_wins(vec![]).with_endings(vec![false, false]);
        let original = progress.clone();
        let hints = EndingHints::new(names(), Some(&achievements()), "english");
        let mut state = Spoilers::default();
        assert!(!state.ending_visible(0, &progress));
        assert!(hints.visible(0, &state).is_empty());
        for expected in 1..=4 {
            state.reveal_next(0, &hints);
            assert_eq!(hints.visible(0, &state).len(), expected);
            assert!(state.ending_visible(0, &progress));
            assert!(!state.ending_visible(1, &progress));
            assert!(!state.ending_total);
            if expected == 1 {
                assert_eq!(hints.visible(0, &state)[0].text, "Look for a way out.");
                assert_eq!(
                    hints.title(0, names(), &progress, &state),
                    "Undiscovered ending"
                );
            } else {
                assert_eq!(
                    hints.title(0, names(), &progress, &state),
                    "Secret destination"
                );
            }
        }
        state.reveal_next(0, &hints);
        assert_eq!(hints.visible(0, &state).len(), 4);
        assert_eq!(progress, original);
    }

    #[test]
    fn persisted_consent_tracks_ending_and_detail_not_catalogue_position() {
        let metadata = scoped_for_test(Metadata {
            endings: vec![ending("first"), ending("second")],
            ..Default::default()
        });
        let hints = EndingHints::new(names(), None, "english");
        let mut state = Spoilers::default();
        state.reveal_next(0, &hints);
        state.layer_total = true;
        let state = Spoilers::from_json(state.to_json()).unwrap();
        drop(metadata);
        let _metadata = scoped_for_test(Metadata {
            endings: vec![ending("second"), ending("first")],
            ..Default::default()
        });
        let hints = EndingHints::new(names(), Some(&achievements()), "english");
        assert!(state.layer_total);
        assert!(!state.ending_total);
        assert!(hints.visible(0, &state).is_empty());
        let revealed = hints.visible(1, &state);
        assert_eq!(revealed.len(), 1);
        assert_eq!(revealed[0].label, "Ending name");
        assert_eq!(hints.next(1, &state).unwrap().label, "Hint");
    }

    #[test]
    fn rendering_does_not_reveal_hints_but_a_click_reveals_exactly_one() {
        let _metadata = scoped_for_test(Metadata {
            endings: vec![ending("first")],
            ..Default::default()
        });
        let hints = EndingHints::new(names(), Some(&achievements()), "english");
        let ctx = egui::Context::default();
        let mut state = Spoilers::default();
        let mut draw = |input| {
            let mut rect = egui::Rect::NOTHING;
            let mut output = ctx.run_ui(input, |ui| {
                rect = ui
                    .scope(|ui| {
                        hint_button(ui, names(), 0, &mut state, &hints);
                    })
                    .response
                    .rect;
            });
            output.textures_delta.clear();
            (rect, hints.visible(0, &state).len())
        };
        let (rect, revealed) = draw(egui::RawInput::default());
        assert_eq!(revealed, 0);
        for pressed in [true, false] {
            draw(egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(rect.center()),
                    egui::Event::PointerButton {
                        pos: rect.center(),
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            });
        }
        let (_, revealed) = draw(egui::RawInput::default());
        assert_eq!(revealed, 1);
    }

    #[test]
    fn completed_progress_does_not_disclose_exhaustion_without_a_request() {
        let _metadata = scoped_for_test(Metadata {
            endings: vec![ending("first")],
            ..Default::default()
        });
        let hints = EndingHints::new(names(), None, "english");
        let progress = GameProgress::from_wins(vec![]).with_endings(vec![true]);
        let mut state = Spoilers::default();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            next_ending(ui, names(), &progress, &mut state, &hints);
        });
        output.textures_delta.clear();
        let texts: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    Some(text.galley.text())
                } else {
                    None
                }
            })
            .collect();
        assert!(texts.contains(&"Reveal a hint for another ending"));
        assert!(!texts.contains(&"No other hidden endings are available."));
        assert_eq!(state, Spoilers::default());
    }

    #[test]
    fn totals_are_independent_of_hints_and_unknown_progress() {
        let _metadata = scoped_for_test(Metadata {
            layers: vec!["Known", "Unreached"],
            endings: vec![ending("first"), ending("second")],
            ..Default::default()
        });
        let known = GameProgress::from_wins(vec![])
            .with_deepest_layer(0)
            .with_endings(vec![true, false]);
        let unknown = GameProgress::unknown("Unavailable".into());
        let mut state = Spoilers::from_json(json!({})).unwrap();
        assert!(state.layer_visible(0, &known));
        assert!(!state.layer_visible(1, &known));
        assert!(state.ending_visible(0, &known));
        assert!(!state.ending_visible(1, &known));
        assert!(
            (0..2).all(|index| !state.layer_visible(index, &unknown)
                && !state.ending_visible(index, &unknown))
        );
        state.ending_total = true;
        assert!(state.ending_visible(1, &unknown));
        assert!(!state.layer_visible(1, &unknown));
        let hints = EndingHints::new(names(), None, "english");
        assert!(hints.visible(1, &state).is_empty());
        state.layer_total = true;
        assert!(state.layer_visible(1, &unknown));
        state.ending_total = false;
        assert!(!state.ending_visible(1, &unknown));
        assert!(Spoilers::from_json(json!({"endingHints": {"first": -1}})).is_err());
    }
}
