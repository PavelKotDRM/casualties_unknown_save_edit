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
            entry.subtitle = id.to_owned();
            entry.text = description.map(strip_rich_text);
            entry
        })
        .collect()
}
