//! Built-in game wiki assembled from the editor's knowledge of the save
//! format and the game's own language files: characters, skills, body stats,
//! limbs, status effects, world settings, presets, layers and items.

use std::collections::HashMap;

use egui::text::LayoutJob;
use egui::{Color32, Key, KeyboardShortcut, Modifiers, RichText, TextFormat, Ui};
use serde_json::Value;

use super::{Names, category_icon, colors, fmt_num, prettify_id, strip_rich_text};
use crate::catalog;
use crate::game_data::{Change, ContentsUse, Effect, GameEffects, ItemStats, Target};
use crate::player_prefs::GameProgress;
use crate::schema::{self, FieldSpec, Good, Kind, RunSettingKind};

/// Number of layer titles (`layertitle1..=12`) in the game's language file.
const LAYER_TITLES: usize = 12;

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
struct Entry {
    title: String,
    subtitle: String,
    text: Option<String>,
    facts: Vec<(String, String)>,
    /// Titled lists of coloured lines, such as the effects of using an item.
    lists: Vec<(String, Vec<(String, Tone)>)>,
}

impl Entry {
    fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: String::new(),
            text: None,
            facts: Vec::new(),
            lists: Vec::new(),
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
}

/// Shows the wiki window when it is open. Works without a loaded save.
pub fn show(
    ctx: &egui::Context,
    state: &mut WikiState,
    names: Names,
    item_stats: &HashMap<String, ItemStats>,
    effects: &GameEffects,
    unlocks: &GameProgress,
) -> Option<egui::Rect> {
    if !state.open {
        return None;
    }
    let sources = Sources {
        names,
        item_stats,
        effects,
        unlocks,
    };
    let mut open = state.open;
    let shown = super::bounded_window(
        ctx,
        format!("📖 {}", names.text("Game wiki")),
        egui::vec2(860.0, 640.0),
        420.0,
    )
    .open(&mut open)
    .show(ctx, |ui| contents(ui, state, &sources));
    state.open = open;
    shown.map(|shown| shown.response.rect)
}

fn contents(ui: &mut Ui, state: &mut WikiState, sources: &Sources) {
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
            egui::ScrollArea::vertical()
                .id_salt("wiki_scroll")
                .auto_shrink(false)
                .show(ui, |ui| article(ui, state, sources, &terms));
        });
    });
}

/// Search results grouped by section, best matches first.
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

fn article(ui: &mut Ui, state: &WikiState, sources: &Sources, terms: &[String]) {
    let names = sources.names;
    if terms.is_empty() {
        let entries = entries(state.section, sources);
        ui.heading(names.text(state.section.title()).to_string());
        if let Some(intro) = intro(state.section) {
            ui.label(names.text(intro).to_string());
        }
        ui.add_space(4.0);
        for entry in &entries {
            entry_card(ui, entry, terms);
        }
        return;
    }
    let groups = search(sources, terms);
    let found: usize = groups.iter().map(|(_, entries)| entries.len()).sum();
    if found == 0 {
        ui.weak(names.text("Nothing found.").to_string());
        return;
    }
    ui.weak(
        names
            .text("{count} results")
            .replace("{count}", &found.to_string()),
    );
    for (section, entries) in &groups {
        ui.heading(names.text(section.title()).to_string());
        for entry in entries {
            entry_card(ui, entry, terms);
        }
        ui.add_space(6.0);
    }
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
                colors::ACCENT,
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

fn intro(section: Section) -> Option<&'static str> {
    Some(match section {
        Section::Characters => {
            "Playable characters. Each one is unlocked by finishing a run with the previous character."
        }
        Section::Endings => {
            "Ways a run can end successfully. An ending stays hidden until you reach it in the game."
        }
        Section::Skills => {
            "Skills grow with experience. Each character has its own starting levels and experience multipliers."
        }
        Section::Body => "Whole-body stats stored in the save's body object.",
        Section::Limbs => "Every limb stores the same set of stats.",
        Section::Conditions => {
            "Status effects (moodles) shown by the game, with its own descriptions."
        }
        Section::RunSettings => {
            "World settings chosen when a run starts. Defaults are the values of the standard preset."
        }
        Section::Presets => "Built-in world presets and the settings they change.",
        Section::Layers => "World layers and the random modifiers a layer can get.",
        Section::Items => {
            "Items known to the game with their stats from the installation. Effects are read from the game's code: amounts are per use unless marked otherwise."
        }
        Section::Liquids => {
            "Liquids that containers can hold, with the effects of drinking or injecting 100 ml, read from the game's code."
        }
    })
}

fn entries(section: Section, sources: &Sources) -> Vec<Entry> {
    match section {
        Section::Characters => characters(sources),
        Section::Endings => endings(sources),
        Section::Skills => skills(sources.names),
        Section::Body => schema::BODY_GROUPS
            .iter()
            .flat_map(|group| {
                group
                    .fields
                    .iter()
                    .map(move |field| field_entry(sources.names, field, group.title))
            })
            .collect(),
        Section::Limbs => limbs(sources.names),
        Section::Conditions => conditions(sources.names),
        Section::RunSettings => run_settings(sources.names),
        Section::Presets => presets(sources.names),
        Section::Layers => layers(sources),
        Section::Items => items(sources),
        Section::Liquids => liquids(sources),
    }
}

fn endings(sources: &Sources) -> Vec<Entry> {
    let names = sources.names;
    let progress = sources.unlocks;
    schema::ENDINGS
        .iter()
        .enumerate()
        .map(|(i, def)| {
            let mut entry = Entry::new(names.ending_if_discovered(i, progress));
            if !progress.is_ending_discovered(i) {
                entry.subtitle = format!("🔒 {}", names.text("Not discovered yet"));
                return entry;
            }
            if progress.is_known() {
                entry.subtitle = format!("🏁 {}", names.text("Discovered"));
            }
            entry.text = Some(names.text(def.how).into_owned());
            entry.facts.push((
                names.text("Steam achievement").into_owned(),
                def.achievement.to_owned(),
            ));
            entry
        })
        .collect()
}

fn characters(sources: &Sources) -> Vec<Entry> {
    let names = sources.names;
    schema::CHARACTERS
        .iter()
        .enumerate()
        .map(|(index, def)| {
            let progress = sources.unlocks;
            let mut entry = Entry::new(names.character_if_unlocked(index, progress));
            let requirement = (index > 0).then(|| {
                (
                    names.text("Unlock").into_owned(),
                    format!(
                        "{} ({})",
                        names.character_unlock_requirement(),
                        names.character_if_unlocked(index - 1, progress)
                    ),
                )
            });
            if !progress.is_unlocked(index) {
                entry.subtitle = format!("🔒 {}", names.text("Locked"));
                entry.facts.extend(requirement);
                return entry;
            }
            entry.subtitle = if index == 0 {
                names.text("Always unlocked").into_owned()
            } else if !sources.unlocks.is_known() {
                names.text("Unlock state unknown").into_owned()
            } else if sources.unlocks.is_unlocked(index) {
                format!("🔓 {}", names.text("Unlocked"))
            } else {
                format!("🔒 {}", names.text("Locked"))
            };
            let text: Vec<_> = [
                names.character_description(index),
                names.character_quirks(index),
            ]
            .into_iter()
            .flatten()
            .collect();
            entry.text = (!text.is_empty()).then(|| text.join("\n\n"));
            for (skill, (level, multiplier)) in schema::SKILLS
                .iter()
                .zip(def.base_skills.iter().zip(def.gain_multipliers))
            {
                entry.facts.push((
                    names.skill(skill),
                    format!(
                        "{} {level} · {} ×{}",
                        names.text("level"),
                        names.text("XP"),
                        fmt_num(multiplier)
                    ),
                ));
            }
            if index > 0 {
                entry.facts.extend(requirement);
            }
            entry.facts.push((
                names.text("Save value").into_owned(),
                format!("character = {index}"),
            ));
            entry
        })
        .collect()
}

fn skills(names: Names) -> Vec<Entry> {
    let mut list: Vec<Entry> = schema::SKILLS
        .iter()
        .map(|skill| {
            let mut entry = Entry::new(names.skill(skill));
            entry.subtitle = skill.key.to_owned();
            entry.text = Some(names.text(skill.help).into_owned());
            entry.facts.push((
                names.text("Save keys").into_owned(),
                format!("{0}, min{0}, exp{0}, {0}GainMult", skill.key),
            ));
            entry
        })
        .collect();
    let mut table = Entry::new(names.text("Experience per level"));
    table.text = Some(
        names
            .text("Total experience needed to reach each level.")
            .into_owned(),
    );
    table.facts = (1..=30)
        .map(|level| {
            (
                format!("{} {level}", names.text("level")),
                schema::experience_for_level(level).to_string(),
            )
        })
        .collect();
    list.push(table);
    list
}

fn field_entry(names: Names, field: &FieldSpec, group: &str) -> Entry {
    let mut entry = Entry::new(names.text(field.label));
    entry.subtitle = format!("{} · {}", names.text(group), field.key);
    entry.text = (!field.help.is_empty()).then(|| names.text(field.help).into_owned());
    let unit = if field.unit.is_empty() {
        String::new()
    } else {
        format!(" {}", field.unit)
    };
    match field.kind {
        Kind::Flag => entry.facts.push((
            names.text("Type").into_owned(),
            names.text("on/off").into_owned(),
        )),
        Kind::Int if !field.min.is_finite() => entry.facts.push((
            names.text("Type").into_owned(),
            names.text("whole number").into_owned(),
        )),
        _ if field.min.is_finite() && field.max.is_finite() => entry.facts.push((
            names.text("Range").into_owned(),
            format!(
                "{} – {}{unit}",
                fmt_num(f64::from(field.min)),
                fmt_num(f64::from(field.max))
            ),
        )),
        _ => entry.facts.push((
            names.text("Type").into_owned(),
            format!("{}{unit}", names.text("number")),
        )),
    }
    let better = match field.good {
        Good::High => Some(names.text("higher is better").into_owned()),
        Good::Low => Some(names.text("lower is better").into_owned()),
        Good::Near(target) => Some(format!(
            "{} {}{unit}",
            names.text("best near"),
            fmt_num(f64::from(target))
        )),
        Good::Neutral => None,
    };
    if let Some(better) = better {
        entry
            .facts
            .push((names.text("Healthy").into_owned(), better));
    }
    entry
}

fn limbs(names: Names) -> Vec<Entry> {
    let mut list_entry = Entry::new(names.text("Limb list"));
    list_entry.text = Some(
        names
            .text("Order in which the save stores limbs (body.limbs).")
            .into_owned(),
    );
    list_entry.facts = schema::LIMBS
        .iter()
        .enumerate()
        .map(|(index, def)| {
            (
                format!("#{index}"),
                format!("{} ({})", names.limb(index), def.object),
            )
        })
        .collect();
    std::iter::once(list_entry)
        .chain(
            schema::LIMB_FIELDS
                .iter()
                .map(|field| field_entry(names, field, "Limb")),
        )
        .collect()
}

fn conditions(names: Names) -> Vec<Entry> {
    let Some(locale) = names.locale else {
        return Vec::new();
    };
    locale
        .moodles()
        .into_iter()
        .map(|(id, name, description)| {
            let mut entry = Entry::new(strip_rich_text(name));
            entry.subtitle = id.to_owned();
            entry.text = description.map(strip_rich_text);
            entry
        })
        .collect()
}

fn run_settings(names: Names) -> Vec<Entry> {
    let defaults = schema::run_settings_for_preset("normal").unwrap_or_default();
    schema::RUN_SETTINGS
        .iter()
        .map(|setting| {
            let mut entry = Entry::new(names.run_setting(setting.id));
            entry.subtitle = format!("{} · {}", names.text(setting.section), setting.id);
            entry.text = names.run_setting_description(setting.id);
            match setting.kind {
                RunSettingKind::Bool => {}
                RunSettingKind::Float { min, max, whole } => entry.facts.push((
                    names.text("Range").into_owned(),
                    format!(
                        "{} – {}{}{}",
                        fmt_num(f64::from(min)),
                        fmt_num(f64::from(max)),
                        setting.postfix,
                        if whole {
                            format!(" ({})", names.text("whole numbers"))
                        } else {
                            String::new()
                        }
                    ),
                )),
                RunSettingKind::Dropdown(options) => entry.facts.push((
                    names.text("Options").into_owned(),
                    (0..options.len())
                        .map(|i| names.run_setting_option(setting.id, i))
                        .collect::<Vec<_>>()
                        .join(", "),
                )),
            }
            let default = defaults
                .iter()
                .find(|v| v.get("Item1").and_then(Value::as_str) == Some(setting.id))
                .and_then(|v| v.get("Item2"));
            if let Some(default) = default {
                entry.facts.push((
                    names.text("Default").into_owned(),
                    format_setting(names, setting.id, default),
                ));
            }
            entry
        })
        .collect()
}

/// Formats a run-setting value the way the game's settings screen shows it.
fn format_setting(names: Names, id: &str, value: &Value) -> String {
    let def = schema::run_setting(id);
    match (value, def.map(|d| d.kind)) {
        (Value::Bool(on), _) => names.text(if *on { "On" } else { "Off" }).into_owned(),
        (Value::Number(n), Some(RunSettingKind::Dropdown(_))) => {
            names.run_setting_option(id, n.as_u64().unwrap_or(0) as usize)
        }
        (Value::Number(n), _) => format!(
            "{}{}",
            fmt_num(n.as_f64().unwrap_or(0.0)),
            def.map_or("", |d| d.postfix)
        ),
        _ => value.to_string(),
    }
}

fn presets(names: Names) -> Vec<Entry> {
    schema::RUN_PRESETS
        .iter()
        .map(|preset| {
            let locale = |key: String| {
                names
                    .locale
                    .and_then(|l| l.other(&key))
                    .map(strip_rich_text)
            };
            let mut entry = Entry::new(
                locale(format!("preset{}", preset.id)).unwrap_or_else(|| preset.name.to_owned()),
            );
            entry.subtitle = preset.id.to_owned();
            entry.text = locale(format!("preset{}dsc", preset.id));
            let values = schema::run_settings_for_preset(preset.id).unwrap_or_default();
            entry.facts = preset
                .overrides
                .iter()
                .filter_map(|(id, _)| {
                    let value = values
                        .iter()
                        .find(|v| v.get("Item1").and_then(Value::as_str) == Some(id))?
                        .get("Item2")?;
                    Some((names.run_setting(id), format_setting(names, id, value)))
                })
                .collect();
            if preset.overrides.is_empty() {
                entry.facts.push((
                    names.text("Settings").into_owned(),
                    names
                        .text("standard values (see World settings)")
                        .into_owned(),
                ));
            }
            entry
        })
        .collect()
}

fn layers(sources: &Sources) -> Vec<Entry> {
    let names = sources.names;
    let mut list: Vec<Entry> = (1..=LAYER_TITLES)
        .filter_map(|layer| {
            let title = names
                .locale
                .and_then(|l| l.other(&format!("layertitle{layer}")))
                .map(str::to_owned)
                .or_else(|| schema::LAYERS.get(layer - 1).map(|s| (*s).to_owned()))?;
            let biome = layer as i64 - 1;
            if !sources.unlocks.is_layer_reached(biome) {
                let mut entry = Entry::new(names.layer_if_reached(biome, sources.unlocks));
                entry.subtitle = format!("🔒 {}", names.text("Not reached yet"));
                return Some(entry);
            }
            let mut lines = title.lines();
            let mut entry = Entry::new(format!("{layer}. {}", lines.next().unwrap_or_default()));
            let rest: Vec<_> = lines.collect();
            entry.text = (!rest.is_empty()).then(|| rest.join("\n"));
            entry.subtitle = format!("biome = {biome}");
            if sources.unlocks.is_known() {
                entry.subtitle = format!("{} · 🔓 {}", entry.subtitle, names.text("Reached"));
            }
            Some(entry)
        })
        .collect();
    if let Some(locale) = names.locale {
        list.extend((0..64).map_while(|i| {
            let name = locale.other(&format!("layermodifier{i}"))?;
            let mut entry = Entry::new(format!("✦ {}", strip_rich_text(name)));
            entry.subtitle = names.text("Layer modifier").into_owned();
            entry.text = locale
                .other(&format!("layermodifier{i}dsc"))
                .map(strip_rich_text);
            Some(entry)
        }));
    }
    list
}

fn items(sources: &Sources) -> Vec<Entry> {
    let names = sources.names;
    let mut list: Vec<(String, Entry)> = catalog::ITEMS
        .iter()
        .map(|def| {
            let stats = sources.item_stats.get(def.id);
            let category = stats.map_or(def.category, |s| s.category.as_str());
            let mut entry = Entry::new(format!(
                "{} {}",
                category_icon(category),
                names.item(def.id)
            ));
            entry.subtitle = format!("{} · {}", names.text(category), def.id);
            entry.text = names.item_description(def.id);
            if let Some(stats) = stats {
                entry.facts.push((
                    names.text("Weight").into_owned(),
                    format!("{} u", fmt_num(f64::from(stats.weight))),
                ));
                let mut push = |label: &str, value: Option<f32>, suffix: &str| {
                    if let Some(value) = value {
                        entry.facts.push((
                            names.text(label).into_owned(),
                            format!("{}{suffix}", fmt_num(f64::from(value))),
                        ));
                    }
                };
                push("Armor", stats.armor, "");
                push("Insulation", stats.insulation, "");
                push("Container capacity", stats.container_capacity, " u");
                push("Max item weight", stats.container_max_item_weight, " u");
                push("Liquid capacity", stats.liquid_capacity, " ml");
                if let Some(slot) = &stats.wear_slot {
                    entry
                        .facts
                        .push((names.text("Worn on").into_owned(), slot.clone()));
                }
                if stats.only_hold_in_hands {
                    entry.facts.push((
                        names.text("Holding").into_owned(),
                        names.text("hands only").into_owned(),
                    ));
                }
            } else {
                if def.capacity > 0.0 {
                    entry.facts.push((
                        names.text("Liquid capacity").into_owned(),
                        format!("{} ml", fmt_num(f64::from(def.capacity))),
                    ));
                }
                if !def.wear_slot.is_empty() {
                    entry
                        .facts
                        .push((names.text("Worn on").into_owned(), def.wear_slot.to_owned()));
                }
            }
            item_effects(&mut entry, sources, def.id);
            (category.to_owned(), entry)
        })
        .collect();
    list.sort_by(|(a, x), (b, y)| a.cmp(b).then_with(|| x.title.cmp(&y.title)));
    list.into_iter().map(|(_, entry)| entry).collect()
}

/// Adds what the game's code says about an item: value, spoiling, crafting
/// qualities and the effects of using it.
fn item_effects(entry: &mut Entry, sources: &Sources, id: &str) {
    let names = sources.names;
    let Some(profile) = sources.effects.items.get(id) else {
        return;
    };
    let mut fact =
        |key: &str, value: String| entry.facts.push((names.text(key).into_owned(), value));
    if let Some(value) = profile.value.filter(|v| *v > 0.0) {
        fact("Value", fmt_num(f64::from(value)));
    }
    if let Some(minutes) = profile.decay_minutes {
        fact("Spoils after", fmt_duration(names, minutes * 60.0));
    }
    if let Some(level) = profile.recognition.filter(|level| *level > 0) {
        fact(
            "Recognised with",
            names
                .text("INT {level}")
                .replace("{level}", &level.to_string()),
        );
    }
    if !profile.qualities.is_empty() {
        fact("Crafting qualities", qualities(names, &profile.qualities));
    }
    if !profile.default_contents.is_empty() {
        let contents: Vec<_> = profile
            .default_contents
            .iter()
            .map(|(liquid, ml)| {
                format!(
                    "{} {} ml",
                    liquid_name(names, sources.effects, liquid),
                    fmt_num(f64::from(*ml))
                )
            })
            .collect();
        fact("Contents", contents.join(", "));
    }
    if let Some(change) = profile.jump_height_change {
        fact("Jump height", signed(change));
    }
    let properties: Vec<_> = [
        (profile.combineable, "can be combined"),
        (profile.ignore_depression, "usable while depressed"),
        (profile.untradeable, "cannot be traded"),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, text)| names.text(text).into_owned())
    .collect();
    if !properties.is_empty() {
        fact("Properties", properties.join(", "));
    }
    for item_use in &profile.uses {
        let heading = if item_use.on_limb {
            "Used on a limb"
        } else {
            "When used"
        };
        let mut lines = Vec::new();
        if let Some(per_use) = item_use
            .condition_per_use
            .filter(|c| *c > 0.0 && *c < 0.999)
        {
            let uses = (1.0 / per_use).round();
            lines.push((
                names
                    .text("≈{count} uses per item")
                    .replace("{count}", &fmt_num(f64::from(uses))),
                Tone::Neutral,
            ));
        }
        if let Some((kind, ml)) = item_use.contents {
            let (text, inject) = match kind {
                ContentsUse::Drink => ("Drinks {ml} ml of its contents", false),
                ContentsUse::Inject => ("Injects {ml} ml of its contents", true),
                ContentsUse::ApplyToLimb => ("Applies {ml} ml of its contents", true),
            };
            lines.push((
                names.text(text).replace("{ml}", &fmt_num(f64::from(ml))),
                Tone::Neutral,
            ));
            let dose = sources.effects.contents_effects(id, ml, inject);
            lines.extend(
                dose.iter()
                    .filter(|e| !e.conditional)
                    .map(|e| effect_line(names, e)),
            );
            item_use_lists(entry, names, heading, lines, &item_use.effects);
            let sometimes: Vec<_> = dose.iter().filter(|e| e.conditional).collect();
            if !sometimes.is_empty() {
                entry.lists.push((
                    names.text("Possible effects of the contents").into_owned(),
                    sometimes
                        .into_iter()
                        .map(|e| effect_line(names, e))
                        .collect(),
                ));
            }
            continue;
        }
        item_use_lists(entry, names, heading, lines, &item_use.effects);
    }
}

/// Adds the lists of one item use: its steady effects and the ones that only
/// happen sometimes.
fn item_use_lists(
    entry: &mut Entry,
    names: Names,
    heading: &str,
    mut lines: Vec<(String, Tone)>,
    effects: &[Effect],
) {
    lines.extend(
        effects
            .iter()
            .filter(|e| !e.conditional)
            .map(|e| effect_line(names, e)),
    );
    let sometimes: Vec<_> = effects
        .iter()
        .filter(|e| e.conditional)
        .map(|e| effect_line(names, e))
        .collect();
    if !lines.is_empty() {
        entry.lists.push((names.text(heading).into_owned(), lines));
    }
    if !sometimes.is_empty() {
        entry
            .lists
            .push((names.text("Possible effects").into_owned(), sometimes));
    }
}

fn liquids(sources: &Sources) -> Vec<Entry> {
    let names = sources.names;
    let effects = sources.effects;
    let mut list: Vec<Entry> = effects
        .liquids
        .iter()
        .map(|(id, liquid)| {
            let mut entry = Entry::new(format!("🧪 {}", liquid_name(names, effects, id)));
            entry.subtitle = id.clone();
            let key = liquid.locale_name.as_deref().unwrap_or(id);
            entry.text = names
                .locale
                .and_then(|l| l.other(&format!("{key}dsc")))
                .map(strip_rich_text);
            if let Some(value) = liquid.value_per_liter.filter(|v| *v > 0.0) {
                entry.facts.push((
                    names.text("Value per litre").into_owned(),
                    fmt_num(f64::from(value)),
                ));
            }
            if liquid.injectable {
                entry.facts.push((
                    names.text("Injectable").into_owned(),
                    names.text("yes").into_owned(),
                ));
            }
            if !liquid.qualities.is_empty() {
                entry.facts.push((
                    names.text("Crafting qualities").into_owned(),
                    qualities(names, &liquid.qualities),
                ));
            }
            let mut found_in: Vec<String> = effects
                .items
                .iter()
                .filter(|(_, item)| item.default_contents.iter().any(|(l, _)| l == id))
                .map(|(item, _)| names.item(item))
                .collect();
            found_in.sort();
            if !found_in.is_empty() {
                entry
                    .facts
                    .push((names.text("Comes in").into_owned(), found_in.join(", ")));
            }
            for (heading, effects) in [
                ("Drinking 100 ml", &liquid.drink),
                ("Injecting or applying 100 ml", &liquid.inject),
            ] {
                item_use_lists(&mut entry, names, heading, Vec::new(), effects);
            }
            entry
        })
        .collect();
    list.sort_by(|a, b| a.title.cmp(&b.title));
    list
}

/// Display name of a liquid from the game's language file.
fn liquid_name(names: Names, effects: &GameEffects, id: &str) -> String {
    let key = effects
        .liquids
        .get(id)
        .and_then(|l| l.locale_name.as_deref())
        .unwrap_or(id);
    names
        .locale
        .and_then(|l| l.other(key))
        .map_or_else(|| prettify_id(id), strip_rich_text)
}

fn qualities(names: Names, list: &[(String, f32)]) -> String {
    list.iter()
        .map(|(id, amount)| {
            let name = names
                .locale
                .and_then(|l| l.other(&format!("cq{id}")))
                .map_or_else(|| prettify_id(id), str::to_owned);
            if (amount - 1.0).abs() < 1e-4 {
                name
            } else {
                format!("{name} ×{}", fmt_num(f64::from(*amount)))
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn signed(value: f32) -> String {
    let text = fmt_num(f64::from(value.abs()));
    if value < 0.0 {
        format!("−{text}")
    } else {
        format!("+{text}")
    }
}

/// A duration in seconds, minutes or hours.
fn fmt_duration(names: Names, seconds: f32) -> String {
    let (template, value) = if seconds < 120.0 {
        ("{n} s", seconds)
    } else if seconds < 2.0 * 3600.0 {
        ("{n} min", seconds / 60.0)
    } else {
        ("{n} h", seconds / 3600.0)
    };
    let value = if value >= 10.0 {
        value.round()
    } else {
        (value * 10.0).round() / 10.0
    };
    names
        .text(template)
        .replace("{n}", &fmt_num(f64::from(value)))
}

/// Label and save-format description of a stat an effect changes.
fn stat_label(names: Names, effect: &Effect) -> (String, Option<&'static FieldSpec>) {
    let stat = effect.stat.as_str();
    let field = match &effect.target {
        Target::Body => schema::body_field(stat),
        Target::Component(_) => None,
        _ => schema::limb_field(stat),
    };
    if let Some(field) = field {
        return (names.text(field.label).into_owned(), Some(field));
    }
    let label = match (&effect.target, stat) {
        (Target::Component(c), "opiateAmount") if c == "Painkillers" => "Opioids",
        (Target::Component(c), "antagonistAmount") if c == "Painkillers" => "Opioid antagonist",
        (Target::Component(c), "opiateTolerance") if c == "Painkillers" => "Opioid tolerance",
        (Target::Component(c), "amount") if c == "SleepingPills" => "Sleeping pills",
        (_, "disinfect") => "Disinfection",
        (_, "vomit") => "Vomiting",
        _ => return (humanize(stat), None),
    };
    (names.text(label).into_owned(), None)
}

/// `bandageSlowAmount` → `Bandage slow amount`.
fn humanize(key: &str) -> String {
    let mut out = String::new();
    for (i, c) in key.chars().enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
        } else if c.is_uppercase() {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// One effect as a readable line, coloured by whether it helps.
fn effect_line(names: Names, effect: &Effect) -> (String, Tone) {
    let (label, field) = stat_label(names, effect);
    let target = match effect.target {
        Target::Limb(index) => format!("{}: ", names.limb(index)),
        Target::EachLimb => format!("{}: ", names.text("All limbs")),
        _ => String::new(),
    };
    // Units carry their own spacing (`" kg"`, `"%"`).
    let unit = field.map_or("", |f| f.unit);
    let rising = match effect.change {
        Change::Add(v) => Some(v > 0.0),
        Change::Multiply(v) => Some(v > 1.0),
        Change::Set(_) | Change::Trigger => None,
    };
    let mut tone = match (field.map(|f| f.good), rising) {
        (Some(Good::High), Some(up)) if up => Tone::Good,
        (Some(Good::High), Some(_)) => Tone::Bad,
        (Some(Good::Low), Some(up)) if up => Tone::Bad,
        (Some(Good::Low), Some(_)) => Tone::Good,
        _ => Tone::Neutral,
    };
    if effect.stat == "disinfect" {
        tone = Tone::Good;
    } else if effect.stat == "vomit" {
        tone = Tone::Bad;
    }
    let value = match effect.change {
        Change::Add(v) => format!(" {}{unit}", signed(v)),
        Change::Multiply(v) => format!(" ×{}", fmt_num(f64::from(v))),
        Change::Set(v) if field.is_some_and(|f| matches!(f.kind, Kind::Flag)) => {
            format!(": {}", names.text(if v != 0.0 { "on" } else { "off" }))
        }
        Change::Set(v) => format!(" → {}{unit}", fmt_num(f64::from(v))),
        Change::Trigger => String::new(),
    };
    let mut line = format!("{target}{label}{value}");
    if let (Change::Add(v), "weightOffset") = (effect.change, effect.stat.as_str()) {
        line += &format!(
            " ({})",
            names.text("≈{kcal} kcal").replace(
                "{kcal}",
                &fmt_num(f64::from((v * CALORIES_PER_WEIGHT).round()))
            )
        );
    }
    if let Some(seconds) = effect.seconds {
        let template = match effect.change {
            Change::Add(_) => "over {time}",
            _ => "for {time}",
        };
        line += &format!(
            " {}",
            names
                .text(template)
                .replace("{time}", &fmt_duration(names, seconds))
        );
    }
    if effect.whole_item {
        line += &format!(" ({})", names.text("whole item"));
    }
    (line, tone)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::InterfaceLanguage;

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
        }
    }

    #[test]
    fn search_finds_entries_across_sections_without_game_files() {
        let names = Names {
            locale: None,
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
        let found = search("milky");
        // Milky itself, and the locked character whose unlock requirement names Milky.
        assert_eq!(found.len(), 2);
        assert!(found[0].0.contains("Milky") && found[0].1.contains("Unlocked"));
        assert_eq!(found[1].0, "🔒 Character #2");
        // Locked characters and unreached layers reveal nothing.
        assert!(search("dune").is_empty());
        assert!(search("dried desert").is_empty());
        assert_eq!(search("deeper gravel").len(), 1);
        // Only the discovered ending shows its name and how it is reached.
        assert_eq!(search("retrieved").len(), 1);
        assert!(search("more info at meeting").is_empty());
        assert!(search("mercenary scarf").is_empty());
        assert_eq!(
            search("ending #3"),
            [(
                "🔒 Ending #3".to_owned(),
                "🔒 Not discovered yet".to_owned()
            )]
        );
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
        };
        let mut rect = None;
        for _ in 0..4 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                rect = show(ui.ctx(), &mut state, names, &stats, &effects, &progress);
            });
            output.textures_delta.clear();
        }
        let rect = rect.expect("wiki window shown");
        assert!(rect.width() <= 900.0, "wiki window grew to {rect:?}");
    }

    #[test]
    fn search_needs_every_term_and_ranks_title_matches_first() {
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
    fn normal_preset_defaults_are_formatted_with_units() {
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
