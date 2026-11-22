//! Items wiki entries.

use super::super::effects;
use super::*;
use std::cmp::Ordering;
use std::collections::HashMap;

struct ItemSortKey {
    id: String,
    name: String,
    group: usize,
    category: String,
    weight: Option<f32>,
    quality: Option<String>,
    drop_chance: Option<f64>,
}

pub(super) fn items(sources: &Sources) -> Vec<Entry> {
    let names = sources.names;
    let pool_sizes = category_pool_sizes(sources);
    let mut list: Vec<(ItemSortKey, Entry)> = catalog::ITEMS
        .iter()
        .filter(|def| {
            sources
                .quality_filter
                .as_ref()
                .is_none_or(|filter| filter.matches_item(sources.effects, def.id))
        })
        .map(|def| {
            let stats = sources.item_stats.get(def.id);
            let category = stats.map_or(def.category, |s| s.category.as_str());
            let pool_size = pool_sizes.get(category).copied().unwrap_or_default();
            let group = catalog::group(def.group);
            let quality = sources
                .effects
                .items
                .get(def.id)
                .filter(|profile| !profile.qualities.is_empty())
                .map(|profile| effects::qualities(names, &profile.qualities));
            let sort_key = ItemSortKey {
                id: def.id.to_owned(),
                name: names.item(def.id),
                group: catalog::group_order(def.group),
                category: category.to_owned(),
                weight: stats.map(|stats| stats.weight),
                quality,
                drop_chance: item_drop_chance(sources, def.id, category, pool_size),
            };
            let mut entry = Entry::new(format!("{} {}", group.icon, names.item(def.id)));
            entry.icon_id = Some(def.id.to_owned());
            entry.subtitle = names.text(group.label).into_owned();
            entry.text = names.item_description(def.id);
            if let Some(filter) = &sources.quality_filter {
                entry.search_alias = Some(filter.text(names));
            }
            entry.facts.push((
                names.text("Game category").into_owned(),
                names.stat(category),
            ));
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
                        .push((names.text("Worn on").into_owned(), names.wear_slot(slot)));
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
                    entry.facts.push((
                        names.text("Worn on").into_owned(),
                        names.wear_slot(def.wear_slot),
                    ));
                }
            }
            let drops = drop_lines(names, sources, def.id, category, pool_size);
            if !drops.is_empty() {
                entry
                    .lists
                    .push((names.text("Drop sources").into_owned(), drops));
            }
            let merchant_stock = merchant_stock_lines(names, sources, def.id);
            if !merchant_stock.is_empty() {
                entry.lists.push((
                    names.text("Merchant stock (Normal preset)").into_owned(),
                    merchant_stock,
                ));
            }
            effects::item_effects(&mut entry, sources, def.id);
            (sort_key, entry)
        })
        .collect();
    list.sort_by(|(a, _), (b, _)| {
        item_sort_order(a, b, sources.item_sort, sources.item_sort_descending)
    });
    list.into_iter().map(|(_, entry)| entry).collect()
}

fn item_sort_order(
    left: &ItemSortKey,
    right: &ItemSortKey,
    sort: ItemSort,
    descending: bool,
) -> Ordering {
    let by_name = || directed(left.name.cmp(&right.name), descending);
    let by_id = || directed(left.id.cmp(&right.id), descending);
    let order = match sort {
        ItemSort::Name => by_name(),
        ItemSort::Group => directed(left.group.cmp(&right.group), descending).then_with(by_name),
        ItemSort::Category => {
            directed(left.category.cmp(&right.category), descending).then_with(by_name)
        }
        ItemSort::Weight => optional_order(
            left.weight.as_ref(),
            right.weight.as_ref(),
            descending,
            f32::total_cmp,
        )
        .then_with(by_name),
        ItemSort::Quality => optional_order(
            left.quality.as_ref(),
            right.quality.as_ref(),
            descending,
            String::cmp,
        )
        .then_with(by_name),
        ItemSort::DropChance => optional_order(
            left.drop_chance.as_ref(),
            right.drop_chance.as_ref(),
            descending,
            f64::total_cmp,
        )
        .then_with(by_name),
    };
    order.then_with(by_id)
}

fn directed(order: Ordering, descending: bool) -> Ordering {
    if descending { order.reverse() } else { order }
}

fn optional_order<T>(
    left: Option<&T>,
    right: Option<&T>,
    descending: bool,
    compare: impl FnOnce(&T, &T) -> Ordering,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => directed(compare(left, right), descending),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn category_pool_sizes(sources: &Sources) -> HashMap<String, usize> {
    let mut sizes = HashMap::new();
    for def in catalog::ITEMS.iter() {
        let category = sources
            .item_stats
            .get(def.id)
            .map_or(def.category, |stats| stats.category.as_str());
        *sizes.entry(category.to_owned()).or_default() += 1;
    }
    sizes
}

fn item_drop_chance(
    sources: &Sources,
    id: &str,
    category: &str,
    category_pool_size: usize,
) -> Option<f64> {
    sources
        .drop_sources
        .iter()
        .filter_map(|source| source.probability_for(id, category, category_pool_size))
        .max_by(f64::total_cmp)
}

fn drop_source_name(names: Names<'_>, source_name: &str) -> String {
    let name = names.game_identifier(source_name);
    if crate::game_data::traders::character(source_name).is_some() {
        format!("{} ({})", name, names.text("if destroyed"))
    } else {
        name
    }
}

fn merchant_stock_lines(
    names: Names<'_>,
    sources: &Sources<'_>,
    item_id: &str,
) -> Vec<(String, Tone)> {
    let Some(amount) = crate::schema::RUN_PRESETS
        .snapshot()
        .iter()
        .find(|preset| preset.id == "normal")
        .and_then(|preset| {
            preset
                .overrides
                .iter()
                .find_map(|(id, value)| (*id == "traderitemamount").then_some(*value))
        })
        .and_then(|value| match value {
            crate::schema::PresetValue::Float(amount) if amount.is_finite() && amount >= 0.0 => {
                Some(amount)
            }
            _ => None,
        })
    else {
        return Vec::new();
    };

    sources
        .drop_sources
        .iter()
        .filter_map(|source| {
            let character = crate::game_data::traders::character(&source.name)?;
            let probability = crate::game_data::traders::stock_probability(
                item_id,
                sources.item_stats,
                amount,
                character,
            )?;
            Some((
                format!(
                    "{}: {} {}",
                    names.game_identifier(&source.name),
                    percent(probability),
                    names.text("overall chance")
                ),
                Tone::Neutral,
            ))
        })
        .collect()
}

fn drop_lines(
    names: Names<'_>,
    sources: &Sources<'_>,
    item_id: &str,
    category: &str,
    category_pool_size: usize,
) -> Vec<(String, Tone)> {
    let mut lines = Vec::new();
    for source in sources.drop_sources {
        let Some(probability) = source.probability_for(item_id, category, category_pool_size)
        else {
            continue;
        };
        let explicit: Vec<_> = source
            .drops
            .iter()
            .filter(|drop| drop.id == item_id)
            .collect();
        let guaranteed: Vec<_> = source
            .always_drops
            .iter()
            .filter(|drop| drop.id == item_id)
            .collect();
        let mut details = Vec::new();
        if !explicit.is_empty() {
            let chances: Vec<_> = explicit
                .iter()
                .map(|drop| {
                    (f64::from(drop.chance) * f64::from(source.drop_chance_multiplier))
                        .clamp(0.0, 1.0)
                })
                .collect();
            let at_least_one = 1.0
                - chances
                    .iter()
                    .fold(1.0, |remaining, chance| remaining * (1.0 - chance));
            let rolls = explicit
                .iter()
                .zip(&chances)
                .map(|(drop, chance)| {
                    format!(
                        "{} ({}–{}% {})",
                        chance_text(drop.chance, source.drop_chance_multiplier, *chance),
                        fmt_num(f64::from(drop.condition_min * 100.0)),
                        fmt_num(f64::from(drop.condition_max * 100.0)),
                        names.text("condition")
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            details.push(format!(
                "{}: {} ({} {})",
                names.text("Explicit rolls"),
                rolls,
                names.text("at least one"),
                percent(at_least_one)
            ));
        }

        if !guaranteed.is_empty() {
            let conditions = guaranteed
                .iter()
                .map(|drop| {
                    format!(
                        "{}–{}%",
                        fmt_num(f64::from(drop.condition_min * 100.0)),
                        fmt_num(f64::from(drop.condition_max * 100.0))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            details.push(format!(
                "{}{} · {} {}",
                names.text("guaranteed"),
                if guaranteed.len() > 1 {
                    format!(" ×{}", guaranteed.len())
                } else {
                    String::new()
                },
                names.text("condition"),
                conditions
            ));
        }

        if let Some(pool_probability) =
            source.category_probability_for(category, category_pool_size)
        {
            let matching_categories = source
                .item_categories
                .iter()
                .filter(|pool| pool.as_str() == category)
                .count();
            details.push(format!(
                "{}: {} ({} {}, {}/{} {} × 1/{} {})",
                names.text("Random category pool"),
                percent(pool_probability),
                source.guaranteed_drop_amount,
                names.text("picks"),
                matching_categories,
                source.item_categories.len(),
                names.text("categories"),
                category_pool_size,
                names.text("eligible items")
            ));
        }
        lines.push((
            format!(
                "{}: {} {} · {}",
                drop_source_name(names, &source.name),
                percent(probability),
                names.text("overall chance"),
                details.join("; ")
            ),
            Tone::Neutral,
        ));
    }
    lines
}

fn chance_text(base: f32, multiplier: f32, effective: f64) -> String {
    if multiplier == 1.0 {
        percent(effective)
    } else {
        format!(
            "{} ({} × {})",
            percent(effective),
            percent(f64::from(base)),
            fmt_num(f64::from(multiplier)),
        )
    }
}

fn percent(value: f64) -> String {
    format!("{}%", fmt_num(value * 100.0))
}

/// Adds what the game's code says about an item: value, spoiling, crafting
/// qualities and the effects of using it.
pub(super) fn liquids(sources: &Sources) -> Vec<Entry> {
    let names = sources.names;
    let effects = sources.effects;
    let mut list: Vec<Entry> = effects
        .liquids
        .iter()
        .filter(|(id, _)| {
            sources
                .quality_filter
                .as_ref()
                .is_none_or(|filter| filter.matches_liquid(effects, id))
        })
        .map(|(id, liquid)| {
            let mut entry = Entry::new(format!("🧪 {}", effects::liquid_name(names, effects, id)));
            entry.subtitle = id.clone();
            if let Some(filter) = &sources.quality_filter {
                entry.search_alias = Some(filter.text(names));
            }
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
                    effects::qualities(names, &liquid.qualities),
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
                effects::item_use_lists(&mut entry, names, heading, Vec::new(), effects);
            }
            entry
        })
        .collect();
    list.sort_by(|a, b| a.title.cmp(&b.title));
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_data::drops::{DropSource, ItemDrop};
    use crate::ui::InterfaceLanguage;

    fn sort_key(
        id: &str,
        name: &str,
        weight: Option<f32>,
        quality: Option<&str>,
        drop_chance: Option<f64>,
    ) -> ItemSortKey {
        ItemSortKey {
            id: id.into(),
            name: name.into(),
            group: 0,
            category: "tools".into(),
            weight,
            quality: quality.map(str::to_owned),
            drop_chance,
        }
    }

    #[test]
    fn descending_sort_reverses_known_values_but_keeps_missing_values_last() {
        let light = sort_key("light", "Light", Some(1.0), Some("Cutting"), Some(0.2));
        let heavy = sort_key("heavy", "Heavy", Some(4.0), Some("Mining"), Some(0.8));
        let missing = sort_key("missing", "Missing", None, None, None);

        assert_eq!(
            item_sort_order(&light, &heavy, ItemSort::Weight, false),
            Ordering::Less
        );
        assert_eq!(
            item_sort_order(&heavy, &light, ItemSort::Weight, true),
            Ordering::Less
        );
        assert_eq!(
            item_sort_order(&heavy, &missing, ItemSort::Weight, true),
            Ordering::Less
        );
        assert_eq!(
            item_sort_order(&heavy, &light, ItemSort::DropChance, true),
            Ordering::Less
        );
        assert_eq!(
            item_sort_order(&light, &heavy, ItemSort::Quality, false),
            Ordering::Less
        );
    }

    #[test]
    fn drop_sort_key_uses_explicit_and_guaranteed_per_source_chances() {
        let names = Names {
            locale: None,
            interface_language: InterfaceLanguage::from_code(None, None),
        };
        let stats = HashMap::new();
        let effects = GameEffects::default();
        let unlocks = GameProgress::unknown("not installed".into());
        let drop_sources = vec![
            DropSource {
                name: "crate".into(),
                drops: vec![
                    ItemDrop {
                        id: "rope".into(),
                        chance: 0.25,
                        condition_min: 0.0,
                        condition_max: 1.0,
                    },
                    ItemDrop {
                        id: "rope".into(),
                        chance: 0.25,
                        condition_min: 0.0,
                        condition_max: 1.0,
                    },
                ],
                always_drops: Vec::new(),
                item_categories: Vec::new(),
                guaranteed_drop_amount: 0,
                drop_chance_multiplier: 1.0,
            },
            DropSource {
                name: "guaranteed".into(),
                drops: Vec::new(),
                always_drops: vec![ItemDrop {
                    id: "knife".into(),
                    chance: 0.0,
                    condition_min: 1.0,
                    condition_max: 1.0,
                }],
                item_categories: Vec::new(),
                guaranteed_drop_amount: 0,
                drop_chance_multiplier: 1.0,
            },
        ];
        let sources = Sources {
            names,
            item_stats: &stats,
            effects: &effects,
            crafting: &[],
            crafting_icons: None,
            crafting_icons_error: None,
            drop_sources: &drop_sources,
            quality_filter: None,
            item_sort: ItemSort::Name,
            item_sort_descending: false,
            unlocks: &unlocks,
            spoilers: Spoilers::default(),
            hints: EndingHints::new(names, None, "english"),
        };

        assert_eq!(
            item_drop_chance(&sources, "rope", "custom", 1),
            Some(0.4375)
        );
        assert_eq!(item_drop_chance(&sources, "knife", "tools", 0), Some(1.0));
        assert_eq!(item_drop_chance(&sources, "water", "liquid", 0), None);
    }
}
