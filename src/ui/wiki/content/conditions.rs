//! Conditions wiki entries.

use super::*;

pub(super) fn conditions(names: Names) -> Vec<Entry> {
    let Some(locale) = names.locale else {
        return Vec::new();
    };
    locale
        .moodles()
        .into_iter()
        .map(|(id, name, description)| {
            let mut entry = Entry::new(strip_rich_text(name));
            entry.search_alias = Some(id.to_owned());
            let moodles = &crate::game_data::runtime::metadata().moodles;
            entry.title_colour = moodles
                .colours
                .get(id)
                .copied()
                .or(moodles.neutral_colour)
                .map(|[red, green, blue]| Color32::from_rgb(red, green, blue));
            entry.text = description.map(strip_rich_text);
            entry
        })
        .collect()
}
