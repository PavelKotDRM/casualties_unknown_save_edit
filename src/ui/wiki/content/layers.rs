//! Layers wiki entries.

use super::*;

pub(super) fn layers(sources: &Sources) -> Vec<Entry> {
    let names = sources.names;
    let mut list: Vec<Entry> = (1..=schema::LAYERS.len())
        .filter(|&layer| sources.spoilers.layer_visible(layer - 1, sources.unlocks))
        .filter_map(|layer| {
            let title = names
                .locale
                .and_then(|l| l.other(&format!("layertitle{layer}")))
                .map(str::to_owned)
                .or_else(|| schema::LAYERS.get(layer - 1).map(|s| (*s).to_owned()))?;
            let biome = layer as i64 - 1;
            if !sources.unlocks.is_known() {
                let mut entry = Entry::new(names.layer_if_reached(biome, sources.unlocks));
                entry.subtitle = names.text("Unlock state unknown").into_owned();
                return Some(entry);
            }
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
