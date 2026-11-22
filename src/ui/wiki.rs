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
use super::{Names, colors, fmt_num, prettify_id, strip_rich_text};
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
    search_all_sections: bool,
    cache: WikiCache,
    hint_ending: Option<usize>,
    quality_filter: Option<QualityFilter>,
    item_sort: ItemSort,
    item_sort_descending: bool,
}

#[derive(Debug, Default)]
struct WikiCache {
    sections: [Option<Vec<Entry>>; Section::ALL.len()],
    query: Option<Vec<String>>,
    search_scope: Option<Section>,
    results: Vec<(Section, Vec<usize>)>,
    width: f32,
    rendered: usize,
    progress: Option<GameProgress>,
    spoilers: Option<Spoilers>,
    hints: Option<EndingHints>,
}

#[derive(Debug, Clone, PartialEq)]
struct QualityFilter {
    id: String,
    label: String,
    amount: f32,
    is_liquid: bool,
    ignored_id: Option<String>,
    click_hint: String,
}

impl QualityFilter {
    fn text(&self, names: Names<'_>) -> String {
        let template = if self.is_liquid {
            "Matching liquid crafting quality: {quality} ≥{amount}"
        } else {
            "Matching crafting quality: {quality} ≥{amount}"
        };
        names
            .text(template)
            .replace("{quality}", &self.label)
            .replace("{amount}", &fmt_num(f64::from(self.amount)))
    }

    fn matches_item(&self, effects: &GameEffects, item_id: &str) -> bool {
        !self.is_liquid
            && self.ignored_id.as_deref() != Some(item_id)
            && effects.items.get(item_id).is_some_and(|item| {
                item.qualities
                    .iter()
                    .any(|(id, amount)| id == &self.id && *amount >= self.amount)
            })
    }

    fn matches_liquid(&self, effects: &GameEffects, liquid_id: &str) -> bool {
        self.is_liquid
            && self.ignored_id.as_deref() != Some(liquid_id)
            && effects.liquids.get(liquid_id).is_some_and(|liquid| {
                liquid
                    .qualities
                    .iter()
                    .any(|(id, amount)| id == &self.id && *amount >= self.amount)
            })
    }
}

fn moodle_icon_id(id: &str) -> String {
    let id = id
        .strip_suffix("Amount")
        .unwrap_or(id)
        .trim_end_matches(|character: char| character.is_ascii_digit())
        .to_ascii_lowercase();
    let id = match id.as_str() {
        "deaf" => "hearingloss",
        "fibrillation" => "arrythmia",
        "materialloss" => "severematerialloss",
        "prosthesis" => "prosthetic",
        "rippedeyes" => "rippedeye",
        "stimulated" => "stimulants",
        _ => id.as_str(),
    };
    format!("moodle:{id}")
}

fn search_scope(state: &WikiState, quality_filter: Option<&QualityFilter>) -> Option<Section> {
    if state.search_all_sections || quality_filter.is_some() {
        None
    } else {
        Some(state.section)
    }
}

fn sections_in_scope(scope: Option<Section>) -> impl Iterator<Item = Section> {
    Section::ALL
        .into_iter()
        .filter(move |section| scope.is_none_or(|selected| selected == *section))
}

impl WikiState {
    /// Call when game data, language, or progression changes.
    pub fn invalidate(&mut self) {
        self.cache = WikiCache::default();
    }

    fn show_quality_matches(&mut self, filter: QualityFilter) {
        self.section = if filter.is_liquid {
            Section::Liquids
        } else {
            Section::Items
        };
        self.search.clear();
        self.quality_filter = Some(filter);
        self.invalidate();
    }

    fn clear_quality_filter(&mut self) {
        self.quality_filter = None;
        self.invalidate();
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
    Crafting,
    Liquids,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum ItemSort {
    #[default]
    Name,
    Group,
    Category,
    Weight,
    Quality,
    DropChance,
}

impl ItemSort {
    const ALL: [Self; 6] = [
        Self::Name,
        Self::Group,
        Self::Category,
        Self::Weight,
        Self::Quality,
        Self::DropChance,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Group => "Wiki group",
            Self::Category => "Game category",
            Self::Weight => "Weight",
            Self::Quality => "Crafting quality",
            Self::DropChance => "Drop chance",
        }
    }
}

impl Section {
    const ALL: [Self; 12] = [
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
        Self::Crafting,
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
            Self::Crafting => "🛠 Crafting",
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

struct CachedTexture(egui::TextureHandle);

impl std::fmt::Debug for CachedTexture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CachedTexture")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TreeNodeKind {
    Item,
    Liquid,
    Recipe,
    Quality,
}

#[derive(Debug)]
struct TreeNode {
    kind: TreeNodeKind,
    title: String,
    detail: Option<String>,
    icon_id: Option<String>,
    icon: Option<CachedTexture>,
    quality_filter: Option<QualityFilter>,
    children: Vec<TreeNode>,
}

impl TreeNode {
    fn new(
        kind: TreeNodeKind,
        title: impl Into<String>,
        detail: Option<String>,
        icon_id: Option<String>,
    ) -> Self {
        Self {
            kind,
            title: title.into(),
            detail,
            icon_id,
            icon: None,
            quality_filter: None,
            children: Vec::new(),
        }
    }

    fn collect_search_text(&self, output: &mut Vec<String>) {
        output.push(fold(&self.title));
        if let Some(id) = &self.icon_id {
            output.push(fold(id));
        }
        if let Some(detail) = &self.detail {
            output.push(fold(detail));
        }
        for child in &self.children {
            child.collect_search_text(output);
        }
    }

    fn contains_quality(&self, filter: &QualityFilter) -> bool {
        self.quality_filter.as_ref().is_some_and(|candidate| {
            candidate.id.eq_ignore_ascii_case(&filter.id)
                && candidate.is_liquid == filter.is_liquid
                && candidate.amount >= filter.amount
        }) || self
            .children
            .iter()
            .any(|child| child.contains_quality(filter))
    }
}

#[derive(Debug)]
struct CraftingTree {
    title: String,
    root: TreeNode,
}

/// One article of the wiki.
#[derive(Debug)]
struct Entry {
    title: String,
    title_colour: Option<Color32>,
    search_alias: Option<String>,
    icon_id: Option<String>,
    icon: Option<CachedTexture>,
    subtitle: String,
    text: Option<String>,
    facts: Vec<(String, String)>,
    /// Titled lists of coloured lines, such as the effects of using an item.
    lists: Vec<(String, Vec<(String, Tone)>)>,
    line_icon_ids: HashMap<String, String>,
    line_icons: HashMap<String, CachedTexture>,
    crafting_tree: Option<CraftingTree>,
    height: Option<f32>,
}

impl Entry {
    fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            title_colour: None,
            search_alias: None,
            icon_id: None,
            icon: None,
            subtitle: String::new(),
            text: None,
            facts: Vec::new(),
            lists: Vec::new(),
            line_icon_ids: HashMap::new(),
            line_icons: HashMap::new(),
            crafting_tree: None,
            height: None,
        }
    }

    /// Relevance of the entry when it contains every search term (`None`
    /// otherwise): matches in the title count most, then the subtitle, then
    /// the rest of the article.
    fn score(&self, terms: &[String]) -> Option<u32> {
        let title = fold(&self.title);
        let subtitle = fold(&self.subtitle);
        let mut body: Vec<String> = self
            .text
            .iter()
            .map(|text| fold(text))
            .chain(self.search_alias.iter().map(|text| fold(text)))
            .chain(self.facts.iter().map(|(k, v)| fold(&format!("{k} {v}"))))
            .chain(self.lists.iter().flat_map(|(heading, lines)| {
                std::iter::once(fold(heading)).chain(lines.iter().map(|(line, _)| fold(line)))
            }))
            .collect();
        if let Some(tree) = &self.crafting_tree {
            tree.root.collect_search_text(&mut body);
        }
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

fn quality_search_filter(query: &str, sources: &Sources<'_>) -> Option<QualityFilter> {
    for is_liquid in [false, true] {
        let template = if is_liquid {
            "Matching liquid crafting quality: {quality} ≥{amount}"
        } else {
            "Matching crafting quality: {quality} ≥{amount}"
        };
        let translated = sources.names.text(template);
        let Some((label, amount)) = parse_quality_template(&translated, query)
            .or_else(|| parse_quality_template(template, query))
        else {
            continue;
        };
        let amount = amount.parse::<f32>().ok()?;
        if !amount.is_finite() || amount < 0.0 {
            return None;
        }
        let label = label.trim();
        let mut ids: Vec<String> = sources
            .effects
            .items
            .values()
            .flat_map(|item| item.qualities.iter().map(|(id, _)| id.clone()))
            .chain(
                sources
                    .effects
                    .liquids
                    .values()
                    .flat_map(|liquid| liquid.qualities.iter().map(|(id, _)| id.clone())),
            )
            .chain(sources.crafting.iter().flat_map(|recipe| {
                recipe
                    .ingredients
                    .iter()
                    .filter_map(|ingredient| ingredient.quality_id.clone())
            }))
            .collect();
        ids.sort();
        ids.dedup();
        let id = ids.into_iter().find(|id| {
            fold(id) == fold(label)
                || sources
                    .names
                    .locale
                    .and_then(|locale| locale.other(&format!("cq{id}")))
                    .map(strip_rich_text)
                    .map_or_else(|| fold(&prettify_id(id)), |name| fold(&name))
                    == fold(label)
        })?;
        let display_label = sources
            .names
            .locale
            .and_then(|locale| locale.other(&format!("cq{id}")))
            .map(strip_rich_text)
            .unwrap_or_else(|| prettify_id(&id));
        return Some(QualityFilter {
            id,
            label: display_label,
            amount,
            is_liquid,
            ignored_id: None,
            click_hint: sources
                .names
                .text("Show matching items or liquids")
                .into_owned(),
        });
    }
    None
}

fn parse_quality_template(template: &str, query: &str) -> Option<(String, String)> {
    const QUALITY: char = '\u{1}';
    const AMOUNT: char = '\u{2}';

    let marked = template
        .replace("{quality}", &QUALITY.to_string())
        .replace("{amount}", &AMOUNT.to_string());
    let quality_position = marked.find(QUALITY)?;
    let amount_position = marked.find(AMOUNT)?;
    let query = fold(query.trim());
    let (first_position, second_position, quality_first) = if quality_position < amount_position {
        (quality_position, amount_position, true)
    } else {
        (amount_position, quality_position, false)
    };
    let prefix = fold(&marked[..first_position]);
    let separator = fold(&marked[first_position + 1..second_position]);
    let suffix = fold(&marked[second_position + 1..]);
    if separator.is_empty() {
        return None;
    }
    let remainder = query.strip_prefix(&prefix)?;
    let (first, remainder) = remainder.split_once(&separator)?;
    let second = remainder.strip_suffix(&suffix)?;
    if quality_first {
        Some((first.to_owned(), second.to_owned()))
    } else {
        Some((second.to_owned(), first.to_owned()))
    }
}

fn search_entry_score(
    section: Section,
    entry: &Entry,
    terms: &[String],
    quality_filter: Option<&QualityFilter>,
    sources: &Sources<'_>,
) -> Option<u32> {
    entry.score(terms).or_else(|| {
        let filter = quality_filter?;
        let matches = match section {
            Section::Items => entry
                .icon_id
                .as_deref()
                .is_some_and(|id| filter.matches_item(sources.effects, id)),
            Section::Liquids => filter.matches_liquid(sources.effects, &entry.subtitle),
            Section::Crafting => entry
                .crafting_tree
                .as_ref()
                .is_some_and(|tree| tree.root.contains_quality(filter)),
            _ => false,
        };
        matches.then_some(100)
    })
}

/// Data the wiki is built from.
struct Sources<'a> {
    names: Names<'a>,
    item_stats: &'a HashMap<String, ItemStats>,
    effects: &'a GameEffects,
    crafting: &'a [crate::game_data::crafting::CraftingRecipe],
    crafting_icons: Option<&'a HashMap<String, crate::game_data::crafting::CraftingIcon>>,
    crafting_icons_error: Option<&'a str>,
    drop_sources: &'a [crate::game_data::drops::DropSource],
    quality_filter: Option<QualityFilter>,
    item_sort: ItemSort,
    item_sort_descending: bool,
    unlocks: &'a GameProgress,
    spoilers: Spoilers,
    hints: EndingHints,
}

pub(crate) struct WikiData<'a> {
    pub names: Names<'a>,
    pub item_stats: &'a HashMap<String, ItemStats>,
    pub effects: &'a GameEffects,
    pub crafting: &'a [crate::game_data::crafting::CraftingRecipe],
    pub crafting_icons: Option<&'a HashMap<String, crate::game_data::crafting::CraftingIcon>>,
    pub crafting_icons_error: Option<&'a str>,
    pub drop_sources: &'a [crate::game_data::drops::DropSource],
    pub unlocks: &'a GameProgress,
}

/// Shows the wiki window when it is open. Works without a loaded save.
pub(crate) fn show(
    ctx: &egui::Context,
    state: &mut WikiState,
    data: WikiData<'_>,
    spoilers: &mut SpoilerUi<'_>,
) -> Option<egui::Rect> {
    if !state.open {
        return None;
    }
    let WikiData {
        names,
        item_stats,
        effects,
        crafting,
        crafting_icons,
        crafting_icons_error,
        drop_sources,
        unlocks,
    } = data;
    let mut sources = Sources {
        names,
        item_stats,
        effects,
        crafting,
        crafting_icons,
        crafting_icons_error,
        drop_sources,
        quality_filter: state.quality_filter.clone(),
        item_sort: state.item_sort,
        item_sort_descending: state.item_sort_descending,
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
    let auto_all_sections = quality_search_filter(&state.search, sources).is_some();
    let search_id = egui::Id::unique("wiki_search");
    if ui.input_mut(|i| i.consume_shortcut(&FOCUS_SEARCH)) {
        ui.memory_mut(|memory| memory.request_focus(search_id));
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("🔍");
        let search_response = ui
            .add(
                egui::TextEdit::singleline(&mut state.search)
                    .id(search_id)
                    .hint_text(
                        names
                            .text(if state.search_all_sections || auto_all_sections {
                                "Search all sections…"
                            } else {
                                "Search this section…"
                            })
                            .to_string(),
                    )
                    .desired_width(260.0),
            )
            .on_hover_text(
                names
                    .text("Ctrl+F to search, Esc to clear. Every word must match. Search is limited to the selected section unless Search all sections is enabled. Crafting-quality queries include matching results from all sections.")
                    .to_string(),
            );
        let mut search_all_sections = state.search_all_sections || auto_all_sections;
        let scope_response = ui.add_enabled(
            !auto_all_sections,
            egui::Checkbox::new(
                &mut search_all_sections,
                names.text("Search all sections"),
            ),
        );
        if scope_response.changed() {
            state.search_all_sections = search_all_sections;
        }
        if (search_response.has_focus() || search_response.lost_focus())
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
                let selected = state.section == section;
                if ui
                    .add_sized(
                        [170.0, ui.spacing().interact_size.y],
                        egui::Button::selectable(selected, names.text(section.title()).to_string()),
                    )
                    .clicked()
                {
                    state.section = section;
                    state.search.clear();
                    if state.quality_filter.is_some() {
                        state.clear_quality_filter();
                        sources.quality_filter = None;
                    }
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
            if matches!(state.section, Section::Crafting | Section::Items)
                && let Some(error) = sources.crafting_icons_error
            {
                ui.colored_label(
                    colors::WARN,
                    names
                        .text("Some game item images could not be loaded.")
                        .to_string(),
                )
                .on_hover_text(error);
            }
            if let Some(filter) = state.quality_filter.clone() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(filter.text(names));
                    if ui.button(names.text("Clear filter")).clicked() {
                        state.clear_quality_filter();
                        sources.quality_filter = None;
                    }
                });
                ui.separator();
            }
            if state.section == Section::Items {
                ui.horizontal_wrapped(|ui| {
                    ui.label(names.text("Sort by"));
                    let old_sort = state.item_sort;
                    egui::ComboBox::from_id_salt("wiki_item_sort")
                        .selected_text(names.text(state.item_sort.label()).to_string())
                        .show_ui(ui, |ui| {
                            for sort in ItemSort::ALL {
                                ui.selectable_value(
                                    &mut state.item_sort,
                                    sort,
                                    names.text(sort.label()).to_string(),
                                );
                            }
                        });
                    if state.item_sort != old_sort {
                        state.invalidate();
                        sources.item_sort = state.item_sort;
                    }
                    let direction = if state.item_sort_descending {
                        "Descending"
                    } else {
                        "Ascending"
                    };
                    let response = ui.button(names.text(direction));
                    let response = if state.item_sort == ItemSort::DropChance {
                        response.on_hover_text(names.text(
                            "Drop chance sort uses the highest per-source probability, including calculated random category-pool odds.",
                        ))
                    } else {
                        response
                    };
                    if response.clicked() {
                        state.item_sort_descending = !state.item_sort_descending;
                        state.invalidate();
                        sources.item_sort_descending = state.item_sort_descending;
                    }
                });
                ui.separator();
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
    let quality_filter = quality_search_filter(&terms.join(" "), sources);
    let mut groups: Vec<(u32, Section, Vec<Entry>)> = Section::ALL
        .into_iter()
        .filter_map(|section| {
            let mut found: Vec<(u32, Entry)> = entries(section, sources)
                .into_iter()
                .filter_map(|entry| {
                    Some((
                        search_entry_score(
                            section,
                            &entry,
                            terms,
                            quality_filter.as_ref(),
                            sources,
                        )?,
                        entry,
                    ))
                })
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
    let quality_filter = quality_search_filter(&state.search, sources);
    let scoped_section = search_scope(state, quality_filter.as_ref());
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
        let mut requested_filter = None;
        ui.heading(names.text(state.section.title()).to_string());
        if let Some(intro) = intro(state.section) {
            ui.label(names.text(intro).to_string());
        }
        ui.add_space(4.0);
        for (index, entry) in entries.iter_mut().enumerate() {
            cache.rendered += virtual_entry(
                ui,
                (state.section as usize, index),
                entry,
                terms,
                sources.crafting_icons,
                &mut requested_filter,
            );
        }
        if let Some(filter) = requested_filter {
            state.show_quality_matches(filter);
        }
        return;
    }
    let mut requested_filter = None;
    if cache.query.as_deref() != Some(terms) || cache.search_scope != scoped_section {
        let mut groups = Vec::new();
        for section in sections_in_scope(scoped_section) {
            let entries =
                cache.sections[section as usize].get_or_insert_with(|| entries(section, sources));
            let mut found: Vec<_> = entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    Some((
                        search_entry_score(
                            section,
                            entry,
                            terms,
                            quality_filter.as_ref(),
                            sources,
                        )?,
                        index,
                    ))
                })
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
        cache.search_scope = scoped_section;
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
                cache.rendered += virtual_entry(
                    ui,
                    (*section as usize, index),
                    &mut entries[index],
                    terms,
                    sources.crafting_icons,
                    &mut requested_filter,
                );
            }
        }
        ui.add_space(6.0);
    }
    if let Some(filter) = requested_filter {
        state.show_quality_matches(filter);
    }
}

fn virtual_entry(
    ui: &mut Ui,
    id: (usize, usize),
    entry: &mut Entry,
    terms: &[String],
    icons: Option<&HashMap<String, crate::game_data::crafting::CraftingIcon>>,
    requested_filter: &mut Option<QualityFilter>,
) -> usize {
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
    ui.push_id(id, |ui| {
        entry_card(ui, entry, terms, icons, requested_filter)
    });
    entry.height = Some(ui.next_widget_position().y - start);
    1
}

/// Text with every occurrence of the search terms highlighted.
fn highlighted(ui: &Ui, text: &str, terms: &[String], color: Color32) -> LayoutJob {
    highlighted_with_style(ui, text, terms, color, egui::TextStyle::Body)
}

fn highlighted_title(ui: &Ui, text: &str, terms: &[String], color: Color32) -> LayoutJob {
    highlighted_with_style(ui, text, terms, color, egui::TextStyle::Heading)
}

fn highlighted_with_style(
    ui: &Ui,
    text: &str,
    terms: &[String],
    color: Color32,
    text_style: egui::TextStyle,
) -> LayoutJob {
    let format = TextFormat {
        font_id: text_style.resolve(ui.style()),
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

fn entry_card(
    ui: &mut Ui,
    entry: &mut Entry,
    terms: &[String],
    icons: Option<&HashMap<String, crate::game_data::crafting::CraftingIcon>>,
    requested_filter: &mut Option<QualityFilter>,
) {
    if entry.icon.is_none()
        && let (Some(id), Some(icon)) = (
            entry.icon_id.as_deref(),
            entry.icon_id.as_ref().and_then(|id| icons?.get(id)),
        )
    {
        entry.icon = Some(CachedTexture(ui.ctx().load_texture(
            format!("wiki-item-{id}"),
            egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            ),
            egui::TextureOptions::NEAREST,
        )));
    }
    for (line, id) in &entry.line_icon_ids {
        if entry.line_icons.contains_key(line) {
            continue;
        }
        if let Some(icon) = icons.and_then(|icons| icons.get(id)) {
            entry.line_icons.insert(
                line.clone(),
                CachedTexture(ui.ctx().load_texture(
                    format!("wiki-effect-{id}"),
                    egui::ColorImage::from_rgba_unmultiplied(
                        [icon.width as usize, icon.height as usize],
                        &icon.rgba,
                    ),
                    egui::TextureOptions::NEAREST,
                )),
            );
        }
    }
    let text_color = ui.visuals().text_color();
    let weak_color = ui.visuals().weak_text_color();
    let title_color = entry.title_colour.unwrap_or(colors::ACCENT);
    let title = highlighted_title(ui, &entry.title, terms, title_color);
    let subtitle =
        (!entry.subtitle.is_empty()).then(|| highlighted(ui, &entry.subtitle, terms, weak_color));
    super::card(ui, |ui| {
        ui.set_width(ui.available_width());
        if let Some(icon) = &entry.icon {
            ui.horizontal_top(|ui| {
                ui.add(egui::Image::new(&icon.0).fit_to_exact_size(egui::vec2(48.0, 48.0)));
                ui.vertical(|ui| {
                    if let Some(colour) = entry.title_colour {
                        ui.label(RichText::new("●").color(colour));
                    }
                    ui.add(egui::Label::new(title));
                    if let Some(subtitle) = subtitle {
                        ui.add(egui::Label::new(subtitle).wrap());
                    }
                });
            });
        } else {
            ui.horizontal_wrapped(|ui| {
                if let Some(colour) = entry.title_colour {
                    ui.label(RichText::new("●").color(colour));
                }
                ui.add(egui::Label::new(title));
                if let Some(subtitle) = subtitle {
                    ui.add(egui::Label::new(subtitle).wrap());
                }
            });
        }
        ui.add_space(3.0);
        if let Some(text) = &entry.text {
            ui.add(egui::Label::new(highlighted(ui, text, terms, text_color)).wrap());
        }
        if !entry.facts.is_empty() {
            ui.add_space(3.0);
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
            ui.add_space(4.0);
            ui.label(RichText::new(heading).strong());
            for (line, tone) in lines {
                let color = match tone {
                    Tone::Good => colors::GOOD,
                    Tone::Bad => colors::BAD,
                    Tone::Neutral => text_color,
                };
                ui.horizontal_top(|ui| {
                    ui.add_space(8.0);
                    if let Some(icon) = entry.line_icons.get(line) {
                        ui.add(egui::Image::new(&icon.0).fit_to_exact_size(egui::vec2(18.0, 18.0)));
                    }
                    ui.add(egui::Label::new(highlighted(ui, line, terms, color)).wrap());
                });
            }
        }
        if let Some(tree) = &mut entry.crafting_tree {
            ui.add_space(6.0);
            ui.label(RichText::new(&tree.title).strong());
            egui::ScrollArea::horizontal()
                .id_salt(("wiki_crafting_tree", entry.title.clone()))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    render_tree_node(
                        ui,
                        &mut tree.root,
                        terms,
                        icons,
                        requested_filter,
                        &mut Vec::new(),
                    );
                });
        }
    });
    ui.add_space(4.0);
}

fn render_tree_node(
    ui: &mut Ui,
    node: &mut TreeNode,
    terms: &[String],
    icons: Option<&HashMap<String, crate::game_data::crafting::CraftingIcon>>,
    requested_filter: &mut Option<QualityFilter>,
    path: &mut Vec<usize>,
) -> egui::Rect {
    if node.icon.is_none()
        && let (Some(id), Some(icon)) = (
            node.icon_id.as_deref(),
            node.icon_id.as_ref().and_then(|id| icons?.get(id)),
        )
    {
        node.icon = Some(CachedTexture(ui.ctx().load_texture(
            format!("wiki-tree-item-{id}"),
            egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            ),
            egui::TextureOptions::NEAREST,
        )));
    }

    ui.vertical(|ui| {
        let edge = match node.kind {
            TreeNodeKind::Item | TreeNodeKind::Liquid => colors::ACCENT,
            TreeNodeKind::Recipe => colors::GOOD,
            TreeNodeKind::Quality => colors::WARN,
        };
        let response = egui::Frame::group(ui.style())
            .stroke(egui::Stroke::new(1.0, edge.gamma_multiply(0.75)))
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.set_width(154.0);
                ui.horizontal_top(|ui| {
                    if let Some(icon) = &node.icon {
                        ui.add(egui::Image::new(&icon.0).fit_to_exact_size(egui::vec2(40.0, 40.0)));
                    } else {
                        ui.add_sized(
                            [40.0, 40.0],
                            egui::Label::new(
                                RichText::new(match node.kind {
                                    TreeNodeKind::Item => "📦",
                                    TreeNodeKind::Liquid => "🧪",
                                    TreeNodeKind::Recipe => "🛠",
                                    TreeNodeKind::Quality => "✧",
                                })
                                .size(26.0),
                            ),
                        );
                    }
                    ui.vertical(|ui| {
                        ui.set_width(100.0);
                        ui.add(egui::Label::new(highlighted(ui, &node.title, terms, edge)).wrap());
                        if let Some(detail) = &node.detail {
                            ui.add(
                                egui::Label::new(highlighted(
                                    ui,
                                    detail,
                                    terms,
                                    ui.visuals().weak_text_color(),
                                ))
                                .wrap(),
                            );
                        }
                    });
                });
            })
            .response;
        if let Some(filter) = &node.quality_filter {
            let response = ui
                .interact(
                    response.rect,
                    ui.scope_id().with(("wiki_quality_filter", path.clone())),
                    egui::Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(&filter.click_hint);
            if response.clicked() {
                *requested_filter = Some(filter.clone());
            }
        }
        let parent_rect = response.rect;
        if !node.children.is_empty() {
            ui.add_space(10.0);
            let child_rects = ui
                .horizontal(|ui| {
                    let mut rects = Vec::with_capacity(node.children.len());
                    for (index, child) in node.children.iter_mut().enumerate() {
                        path.push(index);
                        rects.push(render_tree_node(
                            ui,
                            child,
                            terms,
                            icons,
                            requested_filter,
                            path,
                        ));
                        path.pop();
                        ui.add_space(8.0);
                    }
                    rects
                })
                .inner;
            if let (Some(first), Some(last)) = (child_rects.first(), child_rects.last()) {
                let center_x = parent_rect.center().x;
                let branch_y = (parent_rect.bottom() + first.top()) * 0.5;
                let left = first.center().x.min(center_x);
                let right = last.center().x.max(center_x);
                let stroke = egui::Stroke::new(1.5, edge.gamma_multiply(0.55));
                let painter = ui.painter();
                painter.line_segment(
                    [parent_rect.center_bottom(), egui::pos2(center_x, branch_y)],
                    stroke,
                );
                painter.line_segment(
                    [egui::pos2(left, branch_y), egui::pos2(right, branch_y)],
                    stroke,
                );
                for child in &child_rects {
                    painter.line_segment(
                        [egui::pos2(child.center().x, branch_y), child.center_top()],
                        stroke,
                    );
                }
            }
        }
        parent_rect
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_data::crafting::{CraftingIngredient, CraftingRecipe, RecipeCategory};
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
            crafting: &[],
            crafting_icons: None,
            crafting_icons_error: None,
            drop_sources: &[],
            quality_filter: None,
            item_sort: ItemSort::default(),
            item_sort_descending: false,
            unlocks,
            spoilers: Spoilers::default(),
            hints: EndingHints::new(names, None, "english"),
        }
    }

    #[test]
    fn quality_filters_match_thresholds_exclusions_and_liquid_scope() {
        let mut effects = GameEffects::default();
        effects
            .items
            .entry("scissors".into())
            .or_default()
            .qualities
            .push(("Cutting".into(), 1.5));
        effects
            .liquids
            .entry("acid".into())
            .or_default()
            .qualities
            .push(("Cutting".into(), 2.0));

        let item_filter = QualityFilter {
            id: "Cutting".into(),
            label: "Cutting".into(),
            amount: 1.5,
            is_liquid: false,
            ignored_id: None,
            click_hint: String::new(),
        };
        assert!(item_filter.matches_item(&effects, "scissors"));
        assert!(!item_filter.matches_liquid(&effects, "acid"));
        assert!(
            !QualityFilter {
                amount: 1.6,
                ..item_filter.clone()
            }
            .matches_item(&effects, "scissors")
        );
        assert!(
            !QualityFilter {
                ignored_id: Some("scissors".into()),
                ..item_filter.clone()
            }
            .matches_item(&effects, "scissors")
        );

        let liquid_filter = QualityFilter {
            is_liquid: true,
            amount: 2.0,
            ..item_filter
        };
        assert!(liquid_filter.matches_liquid(&effects, "acid"));
        assert!(!liquid_filter.matches_item(&effects, "scissors"));
    }

    #[test]
    fn wiki_search_can_be_scoped_and_quality_queries_still_search_all_sections() {
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let stats = HashMap::new();
        let unlocks = GameProgress::unknown("not installed".into());
        let mut effects = GameEffects::default();
        effects
            .items
            .entry("rope".into())
            .or_default()
            .qualities
            .push(("Foliage".into(), 1.0));
        let sources = sources_for(names, &stats, &effects, &unlocks);
        let state = WikiState {
            section: Section::Items,
            search: "rope".into(),
            ..Default::default()
        };
        assert_eq!(search_scope(&state, None), Some(Section::Items));
        assert_eq!(
            sections_in_scope(search_scope(&state, None)).collect::<Vec<_>>(),
            [Section::Items]
        );

        let all_sections = WikiState {
            search_all_sections: true,
            ..state
        };
        assert_eq!(search_scope(&all_sections, None), None);
        assert_eq!(
            sections_in_scope(search_scope(&all_sections, None)).count(),
            Section::ALL.len()
        );

        let quality_query = WikiState {
            section: Section::Characters,
            search: "Matching crafting quality: Foliage ≥1".into(),
            ..Default::default()
        };
        let quality_filter = quality_search_filter(&quality_query.search, &sources);
        assert_eq!(search_scope(&quality_query, quality_filter.as_ref()), None);
    }

    #[test]
    fn quality_filter_labels_translate_and_keep_placeholders() {
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::from_code(Some("ru"), None),
        };
        let filter = QualityFilter {
            id: "Foliage".into(),
            label: "Foliage".into(),
            amount: 1.0,
            is_liquid: false,
            ignored_id: None,
            click_hint: String::new(),
        };
        assert_eq!(
            filter.text(names),
            "Требуемое качество для создания: Foliage ≥1"
        );
        assert_eq!(
            QualityFilter {
                is_liquid: true,
                ..filter
            }
            .text(names),
            "Требуемое качество жидкости для создания: Foliage ≥1"
        );
    }

    #[test]
    fn quality_filters_limit_the_items_and_liquids_sections() {
        let _metadata = scoped_for_test(Metadata {
            items: vec![
                catalog::ItemDef {
                    id: "scissors",
                    category: "tools",
                    group: "materials",
                    wear_slot: "",
                    capacity: 0.0,
                },
                catalog::ItemDef {
                    id: "knife",
                    category: "tools",
                    group: "materials",
                    wear_slot: "",
                    capacity: 0.0,
                },
            ],
            ..Default::default()
        });
        let locale = crate::locale::Locale::from_json(r#"{"name":"English","other":{}}"#).unwrap();
        let names = Names {
            locale: Some(&locale),
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let stats = HashMap::new();
        let unlocks = GameProgress::unknown("not installed".into());
        let mut effects = GameEffects::default();
        effects
            .items
            .entry("scissors".into())
            .or_default()
            .qualities
            .push(("Foliage".into(), 1.0));
        effects
            .items
            .entry("knife".into())
            .or_default()
            .qualities
            .push(("Foliage".into(), 2.0));
        effects
            .liquids
            .entry("acid".into())
            .or_default()
            .qualities
            .push(("Foliage".into(), 1.0));
        effects
            .liquids
            .entry("water".into())
            .or_default()
            .qualities
            .push(("Foliage".into(), 2.0));
        let mut sources = sources_for(names, &stats, &effects, &unlocks);

        sources.quality_filter = Some(QualityFilter {
            id: "Foliage".into(),
            label: "Foliage".into(),
            amount: 1.5,
            is_liquid: false,
            ignored_id: None,
            click_hint: String::new(),
        });
        let items = entries(Section::Items, &sources);
        assert_eq!(items.len(), 1);
        assert!(items[0].title.ends_with("Knife"));
        assert!(items[0].matches("Matching crafting quality: Foliage ≥1.5"));

        sources.quality_filter = Some(QualityFilter {
            is_liquid: true,
            ..sources.quality_filter.take().unwrap()
        });
        let liquids = entries(Section::Liquids, &sources);
        assert_eq!(liquids.len(), 1);
        assert_eq!(liquids[0].subtitle, "water");
        assert!(liquids[0].matches("Matching liquid crafting quality: Foliage ≥1.5"));
    }

    #[test]
    fn quality_queries_show_matching_items_and_their_full_recipe_ingredients() {
        let _metadata = scoped_for_test(Metadata {
            items: vec![
                catalog::ItemDef {
                    id: "fern",
                    category: "materials",
                    group: "materials",
                    wear_slot: "",
                    capacity: 0.0,
                },
                catalog::ItemDef {
                    id: "leaf",
                    category: "materials",
                    group: "materials",
                    wear_slot: "",
                    capacity: 0.0,
                },
                catalog::ItemDef {
                    id: "twig",
                    category: "materials",
                    group: "materials",
                    wear_slot: "",
                    capacity: 0.0,
                },
                catalog::ItemDef {
                    id: "rope",
                    category: "materials",
                    group: "materials",
                    wear_slot: "",
                    capacity: 0.0,
                },
                catalog::ItemDef {
                    id: "tool",
                    category: "tools",
                    group: "tools",
                    wear_slot: "",
                    capacity: 0.0,
                },
            ],
            ..Default::default()
        });
        let locale = crate::locale::Locale::from_json(
            r#"{"name":"English","main":{"fern":"Fern","leaf":"Leaf","rope":"Rope","tool":"Tool"},"other":{"cqfoliage":"Foliage"}}"#,
        )
        .unwrap();
        let names = Names {
            locale: Some(&locale),
            interface_language: InterfaceLanguage::from_code(Some("ru"), None),
        };
        let mut effects = GameEffects::default();
        for (id, amount) in [("fern", 1.0), ("leaf", 2.0), ("twig", 0.9)] {
            effects
                .items
                .entry(id.into())
                .or_default()
                .qualities
                .push(("foliage".into(), amount));
        }
        let unlocks = GameProgress::from_wins(Vec::new());
        let stats = HashMap::new();
        let recipe = CraftingRecipe {
            index: 0,
            intelligence: 1,
            category: RecipeCategory::Tools,
            special_known: false,
            is_repair: false,
            result_id: "tool".into(),
            result_is_liquid: false,
            result_amount: 1,
            result_condition: 1.0,
            dont_drain_result_liquid: false,
            ingredients: vec![
                CraftingIngredient {
                    is_liquid: false,
                    specific_id: None,
                    quality_id: Some("foliage".into()),
                    quality_amount: 1.0,
                    minimum_condition: 0.0,
                    destroy_item: true,
                    ignored_id: None,
                    no_max_condition: false,
                },
                CraftingIngredient {
                    is_liquid: false,
                    specific_id: Some("rope".into()),
                    quality_id: None,
                    quality_amount: 0.0,
                    minimum_condition: 0.9,
                    destroy_item: true,
                    ignored_id: None,
                    no_max_condition: false,
                },
                CraftingIngredient {
                    is_liquid: true,
                    specific_id: Some("water".into()),
                    quality_id: None,
                    quality_amount: 0.0,
                    minimum_condition: 100.0,
                    destroy_item: true,
                    ignored_id: None,
                    no_max_condition: false,
                },
            ],
        };
        effects.liquids.entry("water".into()).or_default();
        let mut sources = sources_for(names, &stats, &effects, &unlocks);
        sources.crafting = std::slice::from_ref(&recipe);

        let results = search(
            &sources,
            &search_terms("Matching crafting quality: Foliage ≥1"),
        );
        let items = &results
            .iter()
            .find(|(section, _)| *section == Section::Items)
            .unwrap()
            .1;
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|entry| entry.title.ends_with("Fern")));
        assert!(items.iter().any(|entry| entry.title.ends_with("Leaf")));
        assert!(!items.iter().any(|entry| entry.title.ends_with("Twig")));
        let localized_results = search(
            &sources,
            &search_terms("Требуемое качество для создания: Foliage ≥1"),
        );
        assert_eq!(
            localized_results
                .iter()
                .find(|(section, _)| *section == Section::Items)
                .unwrap()
                .1
                .len(),
            2
        );

        let recipe_entry = &results
            .iter()
            .find(|(section, _)| *section == Section::Crafting)
            .unwrap()
            .1[0];
        let mut tree_text = Vec::new();
        recipe_entry
            .crafting_tree
            .as_ref()
            .unwrap()
            .root
            .collect_search_text(&mut tree_text);
        let tree_text = tree_text.join(" ");
        assert!(tree_text.contains("rope"));
        assert!(tree_text.contains("water"));
    }

    #[test]
    fn quality_filter_navigation_selects_the_right_section_and_clears_search() {
        let filter = QualityFilter {
            id: "Cutting".into(),
            label: "Cutting".into(),
            amount: 1.0,
            is_liquid: true,
            ignored_id: None,
            click_hint: String::new(),
        };
        let mut state = WikiState {
            search: "acid".into(),
            ..Default::default()
        };
        state.cache.query = Some(vec!["acid".into()]);

        state.show_quality_matches(filter.clone());
        assert_eq!(state.section, Section::Liquids);
        assert!(state.search.is_empty());
        assert_eq!(state.quality_filter, Some(filter));
        assert!(state.cache.query.is_none());

        state.clear_quality_filter();
        assert!(state.quality_filter.is_none());
        assert!(state.cache.query.is_none());
    }

    #[test]
    fn search_finds_entries_across_sections_without_game_files() {
        let character = |name| schema::CharacterDef {
            name,
            base_skills: [0; 3],
            gain_multipliers: [1.0; 3],
            height_multiplier: None,
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
    fn item_articles_show_effective_drop_odds_and_guaranteed_drops() {
        let locale = crate::locale::Locale::from_json(
            r#"{"name":"English","main":{"rope":"Climbing Cord","containercrate":"Container crate"},"other":{}}"#,
        )
        .unwrap();
        let names = Names {
            locale: Some(&locale),
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let _metadata = scoped_for_test(Metadata {
            items: vec![
                catalog::ItemDef {
                    id: "rope",
                    category: "custom",
                    group: "materials",
                    wear_slot: "",
                    capacity: 0.0,
                },
                catalog::ItemDef {
                    id: "sisal",
                    category: "custom",
                    group: "materials",
                    wear_slot: "",
                    capacity: 0.0,
                },
            ],
            presets: vec![crate::schema::RunPreset {
                id: "normal",
                name: "Normal",
                overrides: &[("traderitemamount", crate::schema::PresetValue::Float(1.0))],
            }],
            ..Default::default()
        });
        let stats = HashMap::from([
            ("rope".into(), stock_item_stats("custom")),
            ("sisal".into(), stock_item_stats("custom")),
        ]);
        let effects = GameEffects::default();
        let unlocks = GameProgress::unknown("not installed".into());
        let mut sources = sources_for(names, &stats, &effects, &unlocks);
        let drop_sources = vec![
            crate::game_data::drops::DropSource {
                name: "Containercrate".into(),
                drops: vec![
                    crate::game_data::drops::ItemDrop {
                        id: "rope".into(),
                        chance: 0.25,
                        condition_min: 0.1,
                        condition_max: 0.5,
                    },
                    crate::game_data::drops::ItemDrop {
                        id: "rope".into(),
                        chance: 0.25,
                        condition_min: 0.1,
                        condition_max: 0.5,
                    },
                ],
                always_drops: vec![crate::game_data::drops::ItemDrop {
                    id: "rope".into(),
                    chance: 1.0,
                    condition_min: 1.0,
                    condition_max: 1.0,
                }],
                item_categories: vec!["custom".into()],
                guaranteed_drop_amount: 2,
                drop_chance_multiplier: 2.0,
            },
            crate::game_data::drops::DropSource {
                name: "trader1".into(),
                drops: vec![crate::game_data::drops::ItemDrop {
                    id: "rope".into(),
                    chance: 1.0,
                    condition_min: 1.0,
                    condition_max: 1.0,
                }],
                always_drops: Vec::new(),
                item_categories: Vec::new(),
                guaranteed_drop_amount: 0,
                drop_chance_multiplier: 1.0,
            },
        ];
        sources.drop_sources = &drop_sources;

        let article = entries(Section::Items, &sources)
            .into_iter()
            .find(|entry| entry.title.ends_with("Climbing Cord"))
            .expect("rope wiki article");
        assert_eq!(article.title, "🔩 Climbing Cord");
        assert!(!article.subtitle.contains("rope"));
        let drops = article
            .lists
            .iter()
            .find(|(heading, _)| heading == "Drop sources")
            .expect("drop source list");
        assert!(
            drops
                .1
                .iter()
                .any(|(line, _)| line.starts_with("Container crate: "))
        );
        assert!(
            drops
                .1
                .iter()
                .any(|(line, _)| line.contains("at least one 75%"))
        );
        assert!(drops.1.iter().any(|(line, _)| line.contains("guaranteed")));
        assert!(drops.1.iter().any(|(line, _)| line.contains("10–50%")));
        assert!(
            drops
                .1
                .iter()
                .any(|(line, _)| line.contains("overall chance") && line.contains("100%"))
        );
        assert!(
            drops
                .1
                .iter()
                .any(|(line, _)| line.contains("Random category pool: 75%"))
        );
        assert!(
            drops
                .1
                .iter()
                .any(|(line, _)| line.starts_with("Trader 1 (if destroyed): "))
        );
        let merchant_stock = article
            .lists
            .iter()
            .find(|(heading, _)| heading == "Merchant stock (Normal preset)")
            .expect("merchant stock list");
        assert!(
            merchant_stock
                .1
                .iter()
                .any(|(line, _)| line.starts_with("Trader 1: ") && !line.contains("100%"))
        );
    }

    fn stock_item_stats(category: &str) -> crate::game_data::ItemStats {
        crate::game_data::ItemStats {
            category: category.into(),
            weight: 1.0,
            trade_value: 1.0,
            untradeable: false,
            scale_weight_with_condition: false,
            wearable: false,
            wear_slot: None,
            armor: None,
            insulation: None,
            item_tags: Default::default(),
            container_capacity: None,
            container_max_item_weight: None,
            container_encumbrance: None,
            container_tag_restriction: None,
            liquid_capacity: None,
            only_hold_in_hands: false,
            wearable_can_be_held: false,
            desired_wear_limb: None,
            destroy_at_zero_condition: false,
        }
    }

    #[test]
    fn crafting_articles_include_recipe_requirements_and_outputs() {
        let locale = crate::locale::Locale::from_json(
            r#"{"name":"English","main":{"rope":"Rope"},"other":{"cqfoliage":"Foliage"}}"#,
        )
        .unwrap();
        let names = Names {
            locale: Some(&locale),
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let stats = HashMap::new();
        let effects = GameEffects::default();
        let unlocks = GameProgress::from_wins(Vec::new());
        let recipe = CraftingRecipe {
            index: 0,
            intelligence: 1,
            category: RecipeCategory::Materials,
            special_known: false,
            is_repair: false,
            result_id: "rope".into(),
            result_is_liquid: false,
            result_amount: 1,
            result_condition: 1.0,
            dont_drain_result_liquid: false,
            ingredients: vec![CraftingIngredient {
                is_liquid: false,
                specific_id: None,
                quality_id: Some("foliage".into()),
                quality_amount: 1.0,
                minimum_condition: 0.0,
                destroy_item: true,
                ignored_id: Some("rope".into()),
                no_max_condition: false,
            }],
        };
        let mut sources = sources_for(names, &stats, &effects, &unlocks);
        sources.crafting = std::slice::from_ref(&recipe);

        let entries = entries(Section::Crafting, &sources);
        let entry = entries
            .iter()
            .find(|entry| entry.title == "🛠 Rope")
            .unwrap();
        assert_eq!(entry.search_alias.as_deref(), Some("rope foliage"));
        assert_eq!(entry.title, "🛠 Rope");
        assert!(entry.subtitle.contains("INT 1"));
        assert!(
            entry
                .facts
                .iter()
                .any(|(label, value)| { label == "Produces" && value == "Rope · 100% condition" })
        );
        assert!(entry.lists[0].1[0].0.contains("Foliage"));
        assert!(entry.lists[0].1[0].0.contains("quality amount · 1"));
        assert!(entry.lists[0].1[0].0.contains("consumed"));
        assert!(entry.lists[0].1[0].0.contains("not Rope"));
    }

    #[test]
    fn crafting_articles_render_available_game_icons() {
        let icon = crate::game_data::crafting::CraftingIcon {
            width: 1,
            height: 1,
            rgba: vec![255; 4],
        };
        let icons = HashMap::from([("rope".to_owned(), icon.clone()), ("leaf".to_owned(), icon)]);
        let mut entry = Entry::new("🛠 Rope");
        entry.icon_id = Some("rope".into());
        let mut root = TreeNode::new(TreeNodeKind::Item, "Rope", None, Some("rope".into()));
        root.children.push(TreeNode::new(
            TreeNodeKind::Item,
            "Leaf",
            None,
            Some("leaf".into()),
        ));
        entry.crafting_tree = Some(CraftingTree {
            title: "Crafting tree".into(),
            root,
        });
        let context = egui::Context::default();
        let mut output = context.run_ui(Default::default(), |ui| {
            entry_card(ui, &mut entry, &[], Some(&icons), &mut None);
        });
        output.textures_delta.clear();
        assert!(entry.icon.is_some());
        let tree = &entry.crafting_tree.as_ref().unwrap().root;
        assert!(tree.icon.is_some());
        assert!(tree.children[0].icon.is_some());
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
                    WikiData {
                        names,
                        item_stats: &stats,
                        effects: &effects,
                        crafting: &[],
                        crafting_icons: None,
                        crafting_icons_error: None,
                        drop_sources: &[],
                        unlocks: &progress,
                    },
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
        let mut state = WikiState {
            search_all_sections: true,
            ..Default::default()
        };
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
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::English,
        };
        let mut output = ctx.run_ui(Default::default(), |ui| {
            show(
                ui.ctx(),
                &mut state,
                WikiData {
                    names,
                    item_stats: &HashMap::new(),
                    effects: &GameEffects::default(),
                    crafting: &[],
                    crafting_icons: None,
                    crafting_icons_error: None,
                    drop_sources: &[],
                    unlocks: &progress,
                },
                &mut SpoilerUi::new(names, &mut Spoilers::default(), None, "english"),
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
                group: "medical",
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
        let pain_effect = effect(Target::TreatedLimb, "pain", Change::Add(-60.0), None);
        let mut entry = Entry::new("Bandage");
        effects::item_use_lists(&mut entry, names, "When used", Vec::new(), &[pain_effect]);
        assert_eq!(
            entry.line_icon_ids.get("Pain −60").map(String::as_str),
            Some("moodle:pain")
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
    fn effect_icons_use_the_game_moodle_names_for_temperature_and_radiation() {
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let effect = |stat: &str, change| Effect {
            target: Target::Body,
            stat: stat.into(),
            change,
            seconds: None,
            whole_item: false,
            conditional: false,
        };
        let effects = [
            effect("temperature", Change::Add(1.0)),
            effect("temperature", Change::Add(-1.0)),
            effect("radiationSickness", Change::Add(1.0)),
        ];
        let mut entry = Entry::new("Test item");
        effects::item_use_lists(&mut entry, names, "When used", Vec::new(), &effects);
        let icon_for = |effect: &Effect| {
            let line = effect_line(names, effect).0;
            entry.line_icon_ids.get(&line).map(String::as_str)
        };
        assert_eq!(icon_for(&effects[0]), Some("moodle:hot"));
        assert_eq!(icon_for(&effects[1]), Some("moodle:cold"));
        assert_eq!(icon_for(&effects[2]), Some("moodle:irradiated"));
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
        let mut list = entries(Section::Conditions, &sources);
        assert_eq!(list.len(), 4);
        assert_eq!(list[0].title, "Localized pain");
        assert_eq!(list[0].icon_id.as_deref(), Some("moodle:pain"));
        assert!(list.iter().all(|entry| entry.subtitle.is_empty()));
        assert!(list[0].matches("pain2"));
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            for entry in &mut list {
                entry_card(ui, entry, &[], None, &mut None);
            }
        });
        output.textures_delta.clear();
        for (index, native_colour) in native
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
            let colour = list[index].title_colour.expect("status accent colour");
            let actual = [colour.r(), colour.g(), colour.b()];
            let native_max = *native_colour.iter().max().unwrap();
            let native_min = *native_colour.iter().min().unwrap();
            let actual_max = *actual.iter().max().unwrap();
            let actual_min = *actual.iter().min().unwrap();
            assert!(
                actual_max
                    >= if native_max - native_min < 8 {
                        190
                    } else {
                        230
                    }
            );
            let native_saturation =
                f32::from(native_max - native_min) / f32::from(native_max.max(1));
            let actual_saturation =
                f32::from(actual_max - actual_min) / f32::from(actual_max.max(1));
            assert!(actual_saturation >= native_saturation);
            assert_eq!(text.galley.job.sections[0].format.color, colour);
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
