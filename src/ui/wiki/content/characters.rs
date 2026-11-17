//! Characters wiki entries.

use super::*;

pub(super) fn endings(sources: &Sources) -> Vec<Entry> {
    let names = sources.names;
    let progress = sources.unlocks;
    schema::ENDINGS
        .iter()
        .enumerate()
        .filter(|&(i, _)| sources.spoilers.ending_visible(i, progress))
        .map(|(i, def)| {
            let mut entry = Entry::new(sources.hints.title(i, names, progress, &sources.spoilers));
            entry.facts.extend(
                sources
                    .hints
                    .visible(i, &sources.spoilers)
                    .iter()
                    .map(|hint| (names.text(hint.label).into_owned(), hint.text.clone())),
            );
            if !progress.is_known() {
                entry.subtitle = names.text("Unlock state unknown").into_owned();
                return entry;
            }
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

pub(super) fn characters(sources: &Sources) -> Vec<Entry> {
    let names = sources.names;
    schema::CHARACTERS
        .iter()
        .enumerate()
        .map(|(index, def)| {
            let progress = sources.unlocks;
            let mut entry = Entry::new(names.character_if_unlocked(index, progress));
            if !progress.is_known() {
                entry.subtitle = names.text("Unlock state unknown").into_owned();
                return entry;
            }
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

pub(super) fn skills(names: Names) -> Vec<Entry> {
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
                schema::experience_for_level(level).map_or_else(
                    || names.text("Unavailable").into_owned(),
                    |experience| experience.to_string(),
                ),
            )
        })
        .collect();
    list.push(table);
    list
}
