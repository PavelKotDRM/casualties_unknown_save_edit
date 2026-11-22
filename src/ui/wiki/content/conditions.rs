//! Conditions wiki entries.

use super::*;

fn emphasize_colour([red, green, blue]: [u8; 3]) -> Color32 {
    let channels = [red, green, blue].map(f32::from);
    let maximum = channels.iter().copied().fold(0.0, f32::max);
    let minimum = channels.iter().copied().fold(255.0, f32::min);
    if maximum - minimum < 8.0 {
        let shade = maximum.max(190.0) as u8;
        return Color32::from_rgb(shade, shade, shade);
    }

    let luminance = channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
    let saturated = channels.map(|channel| luminance + (channel - luminance) * 1.5);
    let peak = saturated.iter().copied().fold(0.0, f32::max);
    let scale = maximum.max(232.0) / peak.max(1.0);
    let [red, green, blue] = saturated.map(|channel| (channel * scale).clamp(0.0, 255.0) as u8);
    Color32::from_rgb(red, green, blue)
}

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
            entry.icon_id = Some(moodle_icon_id(id));
            let moodles = &crate::game_data::runtime::metadata().moodles;
            entry.title_colour = moodles
                .colours
                .get(id)
                .copied()
                .or(moodles.neutral_colour)
                .map(emphasize_colour);
            entry.text = description.map(strip_rich_text);
            entry
        })
        .collect()
}
