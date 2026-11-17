//! Game-derived item and liquid effect formatting for wiki entries.

use super::*;

pub(super) fn item_effects(entry: &mut Entry, sources: &Sources, id: &str) {
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
pub(super) fn item_use_lists(
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
pub(super) fn liquid_name(names: Names, effects: &GameEffects, id: &str) -> String {
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

pub(super) fn qualities(names: Names, list: &[(String, f32)]) -> String {
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

pub(super) fn signed(value: f32) -> String {
    let text = fmt_num(f64::from(value.abs()));
    if value < 0.0 {
        format!("−{text}")
    } else {
        format!("+{text}")
    }
}

/// A duration in seconds, minutes or hours.
pub(super) fn fmt_duration(names: Names, seconds: f32) -> String {
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
pub(super) fn stat_label(names: Names, effect: &Effect) -> (String, Option<&'static FieldSpec>) {
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
pub(super) fn effect_line(names: Names, effect: &Effect) -> (String, Tone) {
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
