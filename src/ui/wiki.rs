//! Built-in game wiki assembled from the editor's knowledge of the save
//! format and the game's own language files: characters, skills, body stats,
//! limbs, status effects, world settings, presets, layers and items.

use std::collections::HashMap;

mod content;
mod effects;
#[cfg(test)]
use content::run_settings;
use content::{entries, intro};
#[cfg(test)]
use effects::effect_line;

use egui::text::LayoutJob;
use egui::{Color32, Key, KeyboardShortcut, Modifiers, RichText, TextFormat, Ui};
use serde_json::Value;

use super::spoilers::{self, EndingHints, SpoilerUi, Spoilers};
use super::{Names, category_icon, colors, fmt_num, prettify_id, strip_rich_text};
use crate::catalog;
use crate::game_data::{Change, ContentsUse, Effect, GameEffects, ItemStats, Target};
use crate::player_prefs::GameProgress;
use crate::schema::{self, FieldSpec, Good, Kind, RunSettingKind};

/// Calories per unit of the body's `weightOffset`, as the game's `Body.Eat` counts them.
const CALORIES_PER_WEIGHT: f32 = 210.0;

const FOCUS_SEARCH: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F);

/// Open/closed state, selected section and search text of the wiki window.
#[derive(Debug, Default)]
pub struct WikiState {
    /// Whether the window is shown.
    pub open: bool,
    section: Section,
    search: String,
    cache: WikiCache,
    hint_ending: Option<usize>,
}

#[derive(Debug, Default)]
struct WikiCache {
    sections: [Option<Vec<Entry>>; Section::ALL.len()],
    query: Option<Vec<String>>,
    results: Vec<(Section, Vec<usize>)>,
    width: f32,
    rendered: usize,
    progress: Option<GameProgress>,
    spoilers: Option<Spoilers>,
    hints: Option<EndingHints>,
}

impl WikiState {
    /// Call when game data, language, or progression changes.
    pub fn invalidate(&mut self) {
        self.cache = WikiCache::default();
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Section {
    #[default]
    Characters,
    Endings,
    Skills,
    Body,
    Limbs,
    Conditions,
    RunSettings,
    Presets,
    Layers,
    Items,
    Liquids,
}

impl Section {
    const ALL: [Self; 11] = [
        Self::Characters,
        Self::Endings,
        Self::Skills,
        Self::Body,
        Self::Limbs,
        Self::Conditions,
        Self::RunSettings,
        Self::Presets,
        Self::Layers,
        Self::Items,
        Self::Liquids,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::Characters => "👤 Characters",
            Self::Endings => "🏁 Endings",
            Self::Skills => "📈 Skills",
            Self::Body => "❤ Body stats",
            Self::Limbs => "🦴 Limbs",
            Self::Conditions => "🌡 Conditions",
            Self::RunSettings => "⚙ World settings",
            Self::Presets => "🎚 Presets",
            Self::Layers => "🗺 Layers",
            Self::Items => "🎒 Items",
            Self::Liquids => "🧪 Liquids",
        }
    }
}

/// Whether a listed effect helps or harms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    Good,
    Bad,
    Neutral,
}

/// One article of the wiki.
#[derive(Debug)]
struct Entry {
    title: String,
    title_colour: Option<Color32>,
    search_alias: Option<String>,
    subtitle: String,
    text: Option<String>,
    facts: Vec<(String, String)>,
    /// Titled lists of coloured lines, such as the effects of using an item.
    lists: Vec<(String, Vec<(String, Tone)>)>,
    height: Option<f32>,
}

impl Entry {
    fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            title_colour: None,
            search_alias: None,
            subtitle: String::new(),
            text: None,
            facts: Vec::new(),
            lists: Vec::new(),
            height: None,
        }
    }

    /// Relevance of the entry when it contains every search term (`None`
    /// otherwise): matches in the title count most, then the subtitle, then
    /// the rest of the article.
    fn score(&self, terms: &[String]) -> Option<u32> {
        let title = fold(&self.title);
        let subtitle = fold(&self.subtitle);
        let body: Vec<String> = self
            .text
            .iter()
            .map(|text| fold(text))
            .chain(self.search_alias.iter().map(|text| fold(text)))
            .chain(self.facts.iter().map(|(k, v)| fold(&format!("{k} {v}"))))
            .chain(self.lists.iter().flat_map(|(heading, lines)| {
                std::iter::once(fold(heading)).chain(lines.iter().map(|(line, _)| fold(line)))
            }))
            .collect();
        let words: Vec<&str> = title
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect();
        let mut total = 0;
        // The whole query naming the article, ignoring icons and punctuation.
        if !terms.is_empty() && words == terms {
            total += 100;
        }
        for term in terms {
            let term = term.as_str();
            total += if words.contains(&term) {
                100
            } else if words.iter().any(|word| word.starts_with(term)) {
                80
            } else if title.contains(term) {
                60
            } else if subtitle.contains(term) {
                30
            } else if body.iter().any(|text| text.contains(term)) {
                10
            } else {
                return None;
            };
        }
        Some(total)
    }

    #[cfg(test)]
    fn matches(&self, query: &str) -> bool {
        self.score(&search_terms(query)).is_some()
    }
}

/// Lowercases a character and folds `ё` into `е`, so searches ignore case
/// and that spelling difference.
fn fold_char(c: char) -> char {
    let lower = c.to_lowercase().next().unwrap_or(c);
    if lower == 'ё' { 'е' } else { lower }
}

fn fold(text: &str) -> String {
    text.chars().map(fold_char).collect()
}

/// Search terms of a query; an entry must contain all of them.
fn search_terms(query: &str) -> Vec<String> {
    query.split_whitespace().map(fold).collect()
}

/// Data the wiki is built from.
struct Sources<'a> {
    names: Names<'a>,
    item_stats: &'a HashMap<String, ItemStats>,
    effects: &'a GameEffects,
    unlocks: &'a GameProgress,
    spoilers: Spoilers,
    hints: EndingHints,
}

/// Shows the wiki window when it is open. Works without a loaded save.
pub fn show(
    ctx: &egui::Context,
    state: &mut WikiState,
    names: Names,
    item_stats: &HashMap<String, ItemStats>,
    effects: &GameEffects,
    unlocks: &GameProgress,
    spoilers: &mut SpoilerUi<'_>,
) -> Option<egui::Rect> {
    if !state.open {
        return None;
    }
    let mut sources = Sources {
        names,
        item_stats,
        effects,
        unlocks,
        spoilers: spoilers.state.clone(),
        hints: spoilers.hints.clone(),
    };
    let mut open = state.open;
    let shown = super::bounded_window(
        ctx,
        format!("📖 {}", names.text("Game wiki")),
        egui::vec2(860.0, 640.0),
        420.0,
    )
    .open(&mut open)
    .show(ctx, |ui| {
        if item_stats.is_empty() {
            ui.colored_label(
                colors::WARN,
                names
                    .text("Game data unavailable. Select the game folder in Game data.")
                    .to_string(),
            );
        } else {
            contents(ui, state, &mut sources);
        }
    });
    state.open = open;
    *spoilers.state = sources.spoilers;
    shown.map(|shown| shown.response.rect)
}

fn contents(ui: &mut Ui, state: &mut WikiState, sources: &mut Sources) {
    let names = sources.names;
    let search_id = egui::Id::unique("wiki_search");
    if ui.input_mut(|i| i.consume_shortcut(&FOCUS_SEARCH)) {
        ui.memory_mut(|memory| memory.request_focus(search_id));
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("🔍");
        let response = ui
            .add(
                egui::TextEdit::singleline(&mut state.search)
                    .id(search_id)
                    .hint_text(names.text("Search all sections…").to_string())
                    .desired_width(260.0),
            )
            .on_hover_text(
                names
                    .text("Ctrl+F to search, Esc to clear. Every word must match.")
                    .to_string(),
            );
        if (response.has_focus() || response.lost_focus())
            && ui.input(|i| i.key_pressed(Key::Escape))
        {
            state.search.clear();
        }
        if !state.search.is_empty() && ui.button("✖").clicked() {
            state.search.clear();
        }
        if names.locale.is_none() {
            ui.colored_label(
                colors::WARN,
                names
                    .text("Game language files not found: descriptions are limited.")
                    .to_string(),
            );
        }
    });
    ui.separator();
    let terms = search_terms(&state.search);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(170.0);
            for section in Section::ALL {
                let selected = terms.is_empty() && state.section == section;
                if ui
                    .add_sized(
                        [170.0, ui.spacing().interact_size.y],
                        egui::Button::selectable(selected, names.text(section.title()).to_string()),
                    )
                    .clicked()
                {
                    state.section = section;
                    state.search.clear();
                }
            }
        });
        ui.separator();
        // A top-down column: content placed directly in the horizontal row
        // would be laid out left to right without wrapping.
        ui.vertical(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            if matches!(
                state.section,
                Section::Conditions | Section::Items | Section::Liquids
            ) && let Some(error) = &crate::game_data::runtime::metadata().moodles_error
            {
                ui.colored_label(
                    colors::WARN,
                    format!("{} {error}", names.text("Status colours unavailable:")),
                );
            }
            if terms.is_empty() {
                ui.horizontal_wrapped(|ui| match state.section {
                    Section::Endings => {
                        spoilers::ending_total(ui, names, &mut sources.spoilers);
                        spoilers::ending_picker(
                            ui,
                            names,
                            sources.unlocks,
                            &mut sources.spoilers,
                            &sources.hints,
                            &mut state.hint_ending,
                        );
                    }
                    Section::Layers => spoilers::layer_total(ui, names, &mut sources.spoilers),
                    _ => {}
                });
            }
            egui::ScrollArea::vertical()
                .id_salt("wiki_scroll")
                .auto_shrink(false)
                .show(ui, |ui| article(ui, state, sources, &terms));
        });
    });
}

/// Search results grouped by section, best matches first.
#[cfg(test)]
fn search(sources: &Sources, terms: &[String]) -> Vec<(Section, Vec<Entry>)> {
    let mut groups: Vec<(u32, Section, Vec<Entry>)> = Section::ALL
        .into_iter()
        .filter_map(|section| {
            let mut found: Vec<(u32, Entry)> = entries(section, sources)
                .into_iter()
                .filter_map(|entry| Some((entry.score(terms)?, entry)))
                .collect();
            // Stable: equally relevant entries keep the section's order.
            found.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
            let best = found.first()?.0;
            Some((best, section, found.into_iter().map(|(_, e)| e).collect()))
        })
        .collect();
    groups.sort_by_key(|(score, _, _)| std::cmp::Reverse(*score));
    groups
        .into_iter()
        .map(|(_, section, entries)| (section, entries))
        .collect()
}

fn article(ui: &mut Ui, state: &mut WikiState, sources: &Sources, terms: &[String]) {
    let names = sources.names;
    if state.cache.progress.as_ref() != Some(sources.unlocks)
        || state.cache.spoilers.as_ref() != Some(&sources.spoilers)
        || state.cache.hints.as_ref() != Some(&sources.hints)
    {
        state.invalidate();
        state.cache.progress = Some(sources.unlocks.clone());
        state.cache.spoilers = Some(sources.spoilers.clone());
        state.cache.hints = Some(sources.hints.clone());
    }
    let cache = &mut state.cache;
    cache.rendered = 0;
    if (cache.width - ui.available_width()).abs() > 0.5 {
        cache.width = ui.available_width();
        for entry in cache.sections.iter_mut().flatten().flatten() {
            entry.height = None;
        }
    }
    if terms.is_empty() {
        let entries = cache.sections[state.section as usize]
            .get_or_insert_with(|| entries(state.section, sources));
        ui.heading(names.text(state.section.title()).to_string());
        if let Some(intro) = intro(state.section) {
            ui.label(names.text(intro).to_string());
        }
        ui.add_space(4.0);
        for (index, entry) in entries.iter_mut().enumerate() {
            cache.rendered += virtual_entry(ui, (state.section as usize, index), entry, terms);
        }
        return;
    }
    if cache.query.as_deref() != Some(terms) {
        let mut groups = Vec::new();
        for section in Section::ALL {
            let entries =
                cache.sections[section as usize].get_or_insert_with(|| entries(section, sources));
            let mut found: Vec<_> = entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| Some((entry.score(terms)?, index)))
                .collect();
            found.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
            if let Some(&(best, _)) = found.first() {
                groups.push((
                    best,
                    section,
                    found.into_iter().map(|(_, index)| index).collect(),
                ));
            }
        }
        groups.sort_by_key(|(score, _, _)| std::cmp::Reverse(*score));
        cache.results = groups
            .into_iter()
            .map(|(_, section, indices)| (section, indices))
            .collect();
        cache.query = Some(terms.to_vec());
        for entry in cache.sections.iter_mut().flatten().flatten() {
            entry.height = None;
        }
    }
    let found: usize = cache.results.iter().map(|(_, entries)| entries.len()).sum();
    if found == 0 {
        ui.weak(names.text("Nothing found.").to_string());
        return;
    }
    ui.weak(
        names
            .text("{count} results")
            .replace("{count}", &found.to_string()),
    );
    for (section, indices) in &cache.results {
        ui.heading(names.text(section.title()).to_string());
        if let Some(entries) = &mut cache.sections[*section as usize] {
            for &index in indices {
                cache.rendered +=
                    virtual_entry(ui, (*section as usize, index), &mut entries[index], terms);
            }
        }
        ui.add_space(6.0);
    }
}

fn virtual_entry(ui: &mut Ui, id: (usize, usize), entry: &mut Entry, terms: &[String]) -> usize {
    let height = entry.height.unwrap_or(160.0);
    let rect = egui::Rect::from_min_size(
        ui.next_widget_position(),
        egui::vec2(ui.available_width(), height),
    );
    if !ui.clip_rect().intersects(rect) {
        ui.add_space(height);
        return 0;
    }
    let start = ui.next_widget_position().y;
    ui.push_id(id, |ui| entry_card(ui, entry, terms));
    entry.height = Some(ui.next_widget_position().y - start);
    1
}

/// Text with every occurrence of the search terms highlighted.
fn highlighted(ui: &Ui, text: &str, terms: &[String], color: Color32) -> LayoutJob {
    let format = TextFormat {
        font_id: egui::TextStyle::Body.resolve(ui.style()),
        color,
        ..Default::default()
    };
    let chars: Vec<(usize, char)> = text
        .char_indices()
        .map(|(i, c)| (i, fold_char(c)))
        .collect();
    let mut marked = vec![false; chars.len()];
    for term in terms {
        let term: Vec<char> = term.chars().collect();
        if term.is_empty() || term.len() > chars.len() {
            continue;
        }
        for start in 0..=chars.len() - term.len() {
            if chars[start..start + term.len()]
                .iter()
                .zip(&term)
                .all(|((_, c), t)| c == t)
            {
                marked[start..start + term.len()].fill(true);
            }
        }
    }
    let mut job = LayoutJob::default();
    let mut start = 0;
    while start < chars.len() {
        let mark = marked[start];
        let end = (start..chars.len())
            .find(|&i| marked[i] != mark)
            .unwrap_or(chars.len());
        let from = chars[start].0;
        let to = chars.get(end).map_or(text.len(), |(i, _)| *i);
        let mut format = format.clone();
        if mark {
            format.background = colors::ACCENT.gamma_multiply(0.35);
        }
        job.append(&text[from..to], 0.0, format);
        start = end;
    }
    job
}

fn entry_card(ui: &mut Ui, entry: &Entry, terms: &[String]) {
    let text_color = ui.visuals().text_color();
    let weak_color = ui.visuals().weak_text_color();
    super::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::Label::new(highlighted(
                ui,
                &entry.title,
                terms,
                entry.title_colour.unwrap_or(colors::ACCENT),
            )));
            if !entry.subtitle.is_empty() {
                ui.add(egui::Label::new(highlighted(
                    ui,
                    &entry.subtitle,
                    terms,
                    weak_color,
                )));
            }
        });
        if let Some(text) = &entry.text {
            ui.add(egui::Label::new(highlighted(ui, text, terms, text_color)).wrap());
        }
        if !entry.facts.is_empty() {
            ui.add_space(2.0);
        }
        let key_width = (ui.available_width() * 0.35).clamp(80.0, 190.0);
        for (key, value) in &entry.facts {
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(key_width, 0.0),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.set_width(key_width);
                        ui.add(egui::Label::new(RichText::new(key).weak()).wrap());
                    },
                );
                ui.add(egui::Label::new(highlighted(ui, value, terms, text_color)).wrap());
            });
        }
        for (heading, lines) in &entry.lists {
            ui.add_space(2.0);
            ui.label(RichText::new(heading).strong());
            for (line, tone) in lines {
                let color = match tone {
                    Tone::Good => colors::GOOD,
                    Tone::Bad => colors::BAD,
                    Tone::Neutral => text_color,
                };
                ui.horizontal_top(|ui| {
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(highlighted(ui, line, terms, color)).wrap());
                });
            }
        }
    });
    ui.add_space(4.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_data::metadata::Metadata;
    use crate::game_data::runtime::scoped_for_test;
    use crate::ui::InterfaceLanguage;

    fn pain_field() -> FieldSpec {
        FieldSpec {
            key: "pain",
            label: "Pain",
            help: "",
            kind: Kind::Gauge,
            min: 0.0,
            max: 100.0,
            unit: "",
            good: Good::Low,
        }
    }

    fn sources_for<'a>(
        names: Names<'a>,
        stats: &'a HashMap<String, ItemStats>,
        effects: &'a GameEffects,
        unlocks: &'a GameProgress,
    ) -> Sources<'a> {
        Sources {
            names,
            item_stats: stats,
            effects,
            unlocks,
            spoilers: Spoilers::default(),
            hints: EndingHints::new(names, None, "english"),
        }
    }

    #[test]
    fn search_finds_entries_across_sections_without_game_files() {
        let character = |name| schema::CharacterDef {
            name,
            base_skills: [0; 3],
            gain_multipliers: [1.0; 3],
        };
        let ending = |name, how| schema::EndingDef {
            pref_key: name,
            achievement: "",
            name,
            status_key: "",
            status: "",
            how,
        };
        let _metadata = scoped_for_test(Metadata {
            characters: vec![character("Anise"), character("Birch"), character("Cedar")],
            layers: vec!["Surface", "Blue gravel", "Hidden desert"],
            endings: vec![
                ending("Retrieved", "Safe passage"),
                ending("Secret escape", "Secret meeting"),
                ending("Third ending", "Mercenary scarf"),
            ],
            ..Default::default()
        });
        let locale = crate::locale::Locale::from_json(
            r#"{"name":"English","other":{"char1name":"Birch localized"}}"#,
        )
        .unwrap();
        let names = Names {
            locale: Some(&locale),
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let stats = HashMap::new();
        let unlocks = GameProgress::from_wins(vec![true, false])
            .with_deepest_layer(1)
            .with_endings(vec![true, false, false]);
        let effects = GameEffects::default();
        let sources = sources_for(names, &stats, &effects, &unlocks);
        let search = |needle: &str| -> Vec<_> {
            Section::ALL
                .into_iter()
                .flat_map(|s| entries(s, &sources))
                .filter(|e| e.matches(needle))
                .map(|e| (e.title, e.subtitle))
                .collect()
        };
        let found = search("birch localized");
        // The localized character and the locked character requiring it.
        assert_eq!(found.len(), 2);
        assert!(found[0].0.contains("Birch localized") && found[0].1.contains("Unlocked"));
        assert_eq!(found[1].0, "🔒 Character #2");
        // Locked characters and unreached layers reveal nothing.
        assert!(search("cedar").is_empty());
        assert!(search("hidden desert").is_empty());
        assert_eq!(search("blue gravel").len(), 1);
        // Only the discovered ending shows its name and how it is reached.
        assert_eq!(search("retrieved").len(), 1);
        assert!(search("secret meeting").is_empty());
        assert!(search("mercenary scarf").is_empty());
        assert!(search("ending #3").is_empty());
        assert_eq!(entries(Section::Endings, &sources).len(), 1);
        assert_eq!(entries(Section::Layers, &sources).len(), 2);
    }

    #[test]
    fn long_articles_wrap_instead_of_widening_the_window() {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0));
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let stats = HashMap::new();
        let effects = GameEffects::default();
        let progress = GameProgress::from_wins(Vec::new());
        let mut state = WikiState {
            open: true,
            section: Section::Body,
            search: String::new(),
            ..Default::default()
        };
        let mut rect = None;
        let mut spoilers = Spoilers::default();
        for _ in 0..4 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                rect = show(
                    ui.ctx(),
                    &mut state,
                    names,
                    &stats,
                    &effects,
                    &progress,
                    &mut SpoilerUi::new(names, &mut spoilers, None, "english"),
                );
            });
            output.textures_delta.clear();
        }
        let rect = rect.expect("wiki window shown");
        assert!(rect.width() <= 900.0, "wiki window grew to {rect:?}");
    }

    #[test]
    fn cached_search_tracks_explicit_reveals_and_hiding_without_leaking_totals() {
        let _metadata = scoped_for_test(Metadata {
            layers: vec!["Surface", "Secret depths"],
            endings: vec![schema::EndingDef {
                pref_key: "secret",
                achievement: "HIDDEN",
                name: "Secret name",
                status_key: "",
                status: "Secret outcome",
                how: "Classified instructions",
            }],
            ..Default::default()
        });
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::English,
        };
        let stats = HashMap::new();
        let effects = GameEffects::default();
        let progress = GameProgress::from_wins(vec![])
            .with_deepest_layer(0)
            .with_endings(vec![false]);
        let mut sources = sources_for(names, &stats, &effects, &progress);
        let ctx = egui::Context::default();
        let mut state = WikiState::default();
        let mut check = |sources: &Sources, expected| {
            let mut output = ctx.run_ui(Default::default(), |ui| {
                article(ui, &mut state, sources, &search_terms("classified"));
            });
            output.textures_delta.clear();
            assert_eq!(
                state
                    .cache
                    .results
                    .iter()
                    .map(|(_, rows)| rows.len())
                    .sum::<usize>(),
                expected
            );
        };
        check(&sources, 0);
        sources.spoilers = Spoilers::from_json(serde_json::json!({
            "endingHints": {"secret": ["name"]},
        }))
        .unwrap();
        check(&sources, 0);
        sources.spoilers = Spoilers::from_json(serde_json::json!({
            "endingHints": {"secret": ["name", "status", "how"]},
        }))
        .unwrap();
        check(&sources, 1);
        sources.spoilers = Spoilers::default();
        check(&sources, 0);
        assert!(entries(Section::Endings, &sources).is_empty());
        assert_eq!(entries(Section::Layers, &sources).len(), 1);
        sources.spoilers.ending_total = true;
        sources.spoilers.layer_total = true;
        assert_eq!(entries(Section::Endings, &sources).len(), 1);
        assert_eq!(entries(Section::Layers, &sources).len(), 2);
        check(&sources, 0);
        assert!(search(&sources, &search_terms("secret")).is_empty());
    }

    #[test]
    fn large_cached_sections_only_render_visible_cards() {
        let ctx = egui::Context::default();
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::English,
        };
        let stats = HashMap::new();
        let effects = GameEffects::default();
        let progress = GameProgress::from_wins(Vec::new());
        let sources = sources_for(names, &stats, &effects, &progress);
        let mut state = WikiState {
            section: Section::Items,
            ..Default::default()
        };
        state.cache.progress = Some(progress.clone());
        state.cache.spoilers = Some(sources.spoilers.clone());
        state.cache.hints = Some(sources.hints.clone());
        state.cache.sections[Section::Items as usize] = Some(
            (0..10_000)
                .map(|index| {
                    let mut entry = Entry::new(format!("Item {index}"));
                    entry.text = Some("Long description. ".repeat(20));
                    entry
                })
                .collect(),
        );
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| article(ui, &mut state, &sources, &[]));
            });
            output.textures_delta.clear();
            assert!(
                state.cache.rendered < 20,
                "rendered {} cards",
                state.cache.rendered
            );
            assert_eq!(
                state.cache.sections[Section::Items as usize]
                    .as_ref()
                    .unwrap()
                    .len(),
                10_000
            );
        }
        state.invalidate();
        assert!(state.cache.sections.iter().all(Option::is_none));
    }

    #[test]
    fn unavailable_game_keeps_wiki_empty_and_progress_unknown() {
        let _metadata = scoped_for_test(Metadata::default());
        let ctx = egui::Context::default();
        let progress = GameProgress::unknown("Game is not installed".into());
        let mut state = WikiState {
            open: true,
            ..Default::default()
        };
        let mut output = ctx.run_ui(Default::default(), |ui| {
            show(
                ui.ctx(),
                &mut state,
                Names {
                    locale: None,
                    interface_language: InterfaceLanguage::English,
                },
                &HashMap::new(),
                &GameEffects::default(),
                &progress,
                &mut SpoilerUi::new(
                    Names {
                        locale: None,
                        interface_language: InterfaceLanguage::English,
                    },
                    &mut Spoilers::default(),
                    None,
                    "english",
                ),
            );
        });
        output.textures_delta.clear();
        assert!(state.cache.sections.iter().all(Option::is_none));
        assert!(!progress.is_known());
    }

    #[test]
    fn search_needs_every_term_and_ranks_title_matches_first() {
        let _metadata = scoped_for_test(Metadata {
            limb_fields: vec![pain_field()],
            items: vec![catalog::ItemDef {
                id: "bandage",
                category: "medical",
                wear_slot: "",
                capacity: 0.0,
            }],
            ..Default::default()
        });
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let stats = HashMap::new();
        let unlocks = GameProgress::from_wins(Vec::new());
        let mut effects = GameEffects::default();
        effects.items.insert(
            "bandage".into(),
            crate::game_data::ItemProfile {
                uses: vec![crate::game_data::ItemUse {
                    on_limb: true,
                    effects: vec![Effect {
                        target: Target::TreatedLimb,
                        stat: "pain".into(),
                        change: Change::Add(-60.0),
                        seconds: None,
                        whole_item: true,
                        conditional: false,
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            },
        );
        let sources = sources_for(names, &stats, &effects, &unlocks);
        let titles = |query: &str| -> Vec<(Section, String)> {
            search(&sources, &search_terms(query))
                .into_iter()
                .flat_map(|(section, entries)| entries.into_iter().map(move |e| (section, e.title)))
                .collect()
        };

        // The limb stat named "Pain" outranks items that only mention it.
        let pain = titles("PAIN");
        assert_eq!(pain[0], (Section::Limbs, "Pain".to_owned()));
        assert!(pain.iter().any(|(_, title)| title.ends_with("Bandage")));
        // Every word must match, in any order.
        let both = titles("pain bandage");
        assert_eq!(both.len(), 1, "{both:?}");
        assert_eq!(both[0].0, Section::Items);
        assert!(titles("bandage zzz").is_empty());
        assert_eq!(fold("ЁЛКА"), "елка");
    }

    #[test]
    fn effect_lines_are_signed_and_coloured_by_benefit() {
        static BODY_FIELDS: &[FieldSpec] = &[FieldSpec {
            key: "weightOffset",
            label: "Weight",
            help: "",
            kind: Kind::Number,
            min: 0.0,
            max: 100.0,
            unit: " kg",
            good: Good::Neutral,
        }];
        let _metadata = scoped_for_test(Metadata {
            limb_fields: vec![pain_field()],
            limbs: vec![schema::LimbDef {
                object: "Head",
                name: "Head",
            }],
            body_groups: vec![schema::FieldGroup {
                title: "Body",
                icon: "",
                fields: BODY_FIELDS,
            }],
            ..Default::default()
        });
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let effect = |target, stat: &str, change, seconds| Effect {
            target,
            stat: stat.into(),
            change,
            seconds,
            whole_item: false,
            conditional: false,
        };
        assert_eq!(
            effect_line(
                names,
                &effect(Target::TreatedLimb, "pain", Change::Add(-60.0), None)
            ),
            ("Pain −60".to_owned(), Tone::Good)
        );
        let (line, tone) = effect_line(
            names,
            &effect(Target::Limb(0), "pain", Change::Add(10.0), Some(300.0)),
        );
        assert_eq!(line, format!("{}: Pain +10 over 5 min", names.limb(0)));
        assert_eq!(tone, Tone::Bad);
        let (line, _) = effect_line(
            names,
            &effect(Target::Body, "weightOffset", Change::Add(0.5), None),
        );
        assert!(line.ends_with("+0.5 kg (≈105 kcal)"), "{line}");
        let (line, _) = effect_line(
            names,
            &effect(
                Target::Component("Painkillers".into()),
                "opiateAmount",
                Change::Add(140.0),
                None,
            ),
        );
        assert_eq!(line, "Opioids +140");
    }

    #[test]
    fn status_cards_use_localized_names_and_native_colours_including_search() {
        let native = [
            ("pain2", [180, 99, 99]),
            ("energized", [126, 166, 126]),
            ("focused", [110, 129, 177]),
        ];
        let _metadata = scoped_for_test(Metadata {
            moodles: crate::game_data::moodles::MoodleData {
                colours: native
                    .into_iter()
                    .map(|(key, colour)| (key.to_owned(), colour))
                    .collect(),
                neutral_colour: Some([94, 94, 94]),
                ..Default::default()
            },
            ..Default::default()
        });
        let locale = crate::locale::Locale::from_json(
            r#"{"moodles":{"pain2":"<b>Localized pain</b>","pain2dsc":"Pain description","energized":"Localized energy","focused":"Localized focus","focuseddscalt":"Alternate description","unknown":"Unknown intensity"}}"#,
        ).unwrap();
        let names = Names {
            locale: Some(&locale),
            interface_language: InterfaceLanguage::English,
        };
        let stats = HashMap::new();
        let effects = GameEffects::default();
        let progress = GameProgress::from_wins(vec![]);
        let sources = sources_for(names, &stats, &effects, &progress);
        let list = entries(Section::Conditions, &sources);
        assert_eq!(list.len(), 4);
        assert_eq!(list[0].title, "Localized pain");
        assert!(list.iter().all(|entry| entry.subtitle.is_empty()));
        assert!(list[0].matches("pain2"));
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            for entry in &list {
                entry_card(ui, entry, &[]);
            }
        });
        output.textures_delta.clear();
        for (index, colour) in native
            .into_iter()
            .map(|(_, colour)| colour)
            .chain(std::iter::once([94, 94, 94]))
            .enumerate()
        {
            let title = &list[index].title;
            let text = output
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape
                        && text.galley.text() == title
                    {
                        Some(text)
                    } else {
                        None
                    }
                })
                .expect("visible status name");
            assert_eq!(
                text.galley.job.sections[0].format.color,
                Color32::from_rgb(colour[0], colour[1], colour[2])
            );
        }
    }

    #[test]
    fn raw_stat_names_and_runtime_healthy_targets_format_and_colour_effects() {
        let field = |key, min, max, good| FieldSpec {
            key,
            label: key,
            help: "",
            kind: Kind::Gauge,
            min,
            max,
            unit: "",
            good,
        };
        let fields = Box::leak(
            vec![
                field("pain", 0.0, 100.0, Good::Near(0.0)),
                field("brainHealth", 0.0, 100.0, Good::Near(100.0)),
                field("temperature", 20.0, 50.0, Good::Near(37.0)),
                field("stamina", f32::NEG_INFINITY, f32::INFINITY, Good::Neutral),
                field("caffeinated", 0.0, 100.0, Good::Near(0.0)),
                field("focusedLevel", 0.0, 100.0, Good::Near(0.0)),
                field(
                    "strokeAmount",
                    f32::NEG_INFINITY,
                    f32::INFINITY,
                    Good::Neutral,
                ),
            ]
            .into_boxed_slice(),
        );
        let _metadata = scoped_for_test(Metadata {
            body_groups: vec![schema::FieldGroup {
                title: "Body",
                icon: "",
                fields,
            }],
            moodles: crate::game_data::moodles::MoodleData {
                directions: [
                    ("Body.stamina".to_owned(), Good::High),
                    ("Body.caffeinated".to_owned(), Good::High),
                    ("Body.focusedLevel".to_owned(), Good::Neutral),
                ]
                .into(),
                ..Default::default()
            },
            ..Default::default()
        });
        let locale =
            crate::locale::Locale::from_json(r#"{"moodles":{"stroke":"Localized stroke"}}"#)
                .unwrap();
        let names = Names {
            locale: Some(&locale),
            interface_language: InterfaceLanguage::English,
        };
        let effect = |stat: &str, change| Effect {
            target: Target::Body,
            stat: stat.into(),
            change,
            seconds: None,
            whole_item: false,
            conditional: false,
        };
        for (stat, change, expected, tone) in [
            ("pain", Change::Add(-5.0), "Pain −5", Tone::Good),
            ("pain", Change::Add(5.0), "Pain +5", Tone::Bad),
            (
                "brainHealth",
                Change::Add(5.0),
                "Brain health +5",
                Tone::Good,
            ),
            (
                "brainHealth",
                Change::Multiply(0.5),
                "Brain health ×0.5",
                Tone::Bad,
            ),
            (
                "temperature",
                Change::Add(1.0),
                "Temperature +1",
                Tone::Neutral,
            ),
            ("stamina", Change::Add(10.0), "Stamina +10", Tone::Good),
            (
                "caffeinated",
                Change::Add(5.0),
                "Caffeinated +5",
                Tone::Good,
            ),
            (
                "strokeAmount",
                Change::Add(-5.0),
                "Localized stroke −5",
                Tone::Neutral,
            ),
            ("pain", Change::Add(0.0), "Pain +0", Tone::Neutral),
            (
                "focusedLevel",
                Change::Add(5.0),
                "Focused +5",
                Tone::Neutral,
            ),
            ("pain", Change::Multiply(1.0), "Pain ×1", Tone::Neutral),
        ] {
            assert_eq!(
                effect_line(names, &effect(stat, change)),
                (expected.to_owned(), tone)
            );
        }
    }

    #[test]
    fn effect_labels_fall_back_to_the_selected_interface_language() {
        let _metadata = scoped_for_test(Metadata {
            body_groups: vec![schema::FieldGroup {
                title: "Body",
                icon: "",
                fields: &[FieldSpec {
                    key: "happiness",
                    label: "happiness",
                    help: "",
                    kind: Kind::Gauge,
                    min: 0.0,
                    max: 100.0,
                    unit: "",
                    good: Good::High,
                }],
            }],
            ..Default::default()
        });
        let locale =
            crate::locale::Locale::from_json(r#"{"moodles":{"pain":"Боль из игры"}}"#).unwrap();
        let names = Names {
            locale: Some(&locale),
            interface_language: InterfaceLanguage::Russian,
        };
        for (target, stat, amount, english, russian) in [
            (
                Target::Body,
                "happiness",
                -5.0,
                "Happiness −5",
                "Настроение −5",
            ),
            (
                Target::Body,
                "septicShock",
                -25.0,
                "Septic shock −25",
                "Септический шок −25",
            ),
            (
                Target::Body,
                "antibioticImmunityTime",
                2500.0,
                "Antibiotic immunity time +2500",
                "Время защиты антибиотиками +2500",
            ),
            (
                Target::TreatedLimb,
                "muscleHealth",
                5.0,
                "Muscle health +5",
                "Состояние мышц +5",
            ),
        ] {
            let effect = Effect {
                target,
                stat: stat.into(),
                change: Change::Add(amount),
                seconds: None,
                whole_item: false,
                conditional: false,
            };
            assert_eq!(effect_line(names, &effect).0, russian);
            assert_eq!(
                effect_line(
                    Names {
                        interface_language: InterfaceLanguage::English,
                        ..names
                    },
                    &effect,
                )
                .0,
                english,
            );
        }
        let effect = Effect {
            target: Target::TreatedLimb,
            stat: "pain".into(),
            change: Change::Add(-5.0),
            seconds: None,
            whole_item: false,
            conditional: false,
        };
        assert_eq!(effect_line(names, &effect).0, "Боль из игры −5");
    }

    #[test]
    fn normal_preset_defaults_are_formatted_with_units() {
        let _metadata = scoped_for_test(Metadata {
            run_settings: vec![schema::RunSettingDef {
                id: "timelimit",
                label: "Time limit",
                kind: RunSettingKind::Float {
                    min: 0.0,
                    max: 120.0,
                    whole: true,
                },
                postfix: "m",
                section: "World",
            }],
            presets: vec![schema::RunPreset {
                id: "normal",
                name: "Normal",
                overrides: &[("timelimit", schema::PresetValue::Float(60.0))],
            }],
            ..Default::default()
        });
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let time_limit = run_settings(names)
            .into_iter()
            .find(|e| e.subtitle.ends_with("timelimit"))
            .unwrap();
        assert!(
            time_limit
                .facts
                .iter()
                .any(|(k, v)| k == "Default" && v == "60m")
        );
    }
}
