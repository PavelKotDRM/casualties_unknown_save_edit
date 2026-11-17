//! Items wiki entries.

use super::super::effects;
use super::*;

pub(super) fn items(sources: &Sources) -> Vec<Entry> {
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
            effects::item_effects(&mut entry, sources, def.id);
            (category.to_owned(), entry)
        })
        .collect();
    list.sort_by(|(a, x), (b, y)| a.cmp(b).then_with(|| x.title.cmp(&y.title)));
    list.into_iter().map(|(_, entry)| entry).collect()
}

/// Adds what the game's code says about an item: value, spoiling, crafting
/// qualities and the effects of using it.
pub(super) fn liquids(sources: &Sources) -> Vec<Entry> {
    let names = sources.names;
    let effects = sources.effects;
    let mut list: Vec<Entry> = effects
        .liquids
        .iter()
        .map(|(id, liquid)| {
            let mut entry = Entry::new(format!("🧪 {}", effects::liquid_name(names, effects, id)));
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
