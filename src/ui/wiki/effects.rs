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
    if let Some(charge) = profile.max_charge {
        fact("Max charge", fmt_num(f64::from(charge)));
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
            add_effect_icons(entry, names, dose.iter());
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
    add_effect_icons(entry, names, effects.iter());
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

fn add_effect_icons<'a>(
    entry: &mut Entry,
    names: Names<'_>,
    effects: impl IntoIterator<Item = &'a Effect>,
) {
    for effect in effects {
        let line = effect_line(names, effect).0;
        entry
            .line_icon_ids
            .insert(line, effect_moodle_icon_id(effect));
    }
}

pub(super) fn effect_moodle_icon_id(effect: &Effect) -> String {
    let field = effect_moodle_field(effect);
    if let Some(statuses) = field.as_deref().and_then(|field| {
        crate::game_data::runtime::metadata()
            .moodles
            .field_moodles
            .get(field)
    }) && let Some(status) = status_for_effect(effect, statuses)
    {
        return super::moodle_icon_id(status);
    }

    let stat = effect.stat.to_ascii_lowercase();
    let alias = match stat.as_str() {
        "antagonistamount" | "opiateamount" => "drugoverdose",
        "opiatetolerance" => "withdrawal",
        "botheyesgone" => "rippedeyes",
        "bloodoxygen" => "oxygen",
        "brainhealth" => "braindamage",
        "braingrowsickness" => "rnrs",
        "disinfect" | "dirtyness" => "dirty",
        "dislocationtimer" => "dislocation",
        "eyegone" => "rippedeye",
        "infectionamount" => "infected",
        "internalbleeding" => "internalBleed",
        "happiness" => match effect_direction(effect.change) {
            Some(false) => "miserable",
            _ => "happy",
        },
        "radiationsickness" | "irradiated" => "irradiated",
        "septicshock" => "sepsis",
        "sicknessamount" | "vomit" => "sick",
        "snowamount" => "cold",
        "stimulantmultiplier" => "stimulated",
        "usedneuralbooster" => "focused",
        "usingsleepingbag" | "sleeping" => "asleep",
        "venomtotal" => "venom",
        "temperature" => {
            return match temperature_direction(effect.change) {
                Some(true) => "moodle:hot".to_owned(),
                Some(false) => "moodle:cold".to_owned(),
                None => super::moodle_icon_id(&effect.stat),
            };
        }
        _ => &effect.stat,
    };
    super::moodle_icon_id(alias)
}

pub(super) fn effect_moodle_field(effect: &Effect) -> Option<String> {
    let stat = effect.stat.to_ascii_lowercase();
    match stat.as_str() {
        "adrenaline" => Some("Body.curAdrenaline".to_owned()),
        "antibioticimmunitytime" => Some("Body.immunity".to_owned()),
        "bloodpressurechangefrommedicine" => Some("Body.bloodPressure".to_owned()),
        "bleedamount" => Some("Body.totalBleedSpeed".to_owned()),
        _ => match effect.target {
            Target::Body => Some(format!("Body.{}", effect.stat)),
            Target::TreatedLimb | Target::Limb(_) | Target::EachLimb => {
                Some(format!("Limb.{}", effect.stat))
            }
            Target::Component(_) => None,
        },
    }
}

fn status_for_effect<'a>(effect: &Effect, statuses: &'a [String]) -> Option<&'a str> {
    let stat = effect.stat.to_ascii_lowercase();
    let preferred = match stat.as_str() {
        "temperature" => Some(match temperature_direction(effect.change)? {
            true => "hot",
            false => "cold",
        }),
        "heartrate" => Some(match effect_direction(effect.change) {
            Some(true) => "tachycardia",
            Some(false) => "bradycardia",
            None => "palpitations",
        }),
        "bloodpressurechangefrommedicine" => Some(match effect_direction(effect.change) {
            Some(true) => "hypertension",
            Some(false) => "hypotension",
            None => "hypertension",
        }),
        "antibioticimmunitytime" => Some(match effect_direction(effect.change) {
            Some(true) => "highimmunity",
            Some(false) => "lowimmunity",
            None => "highimmunity",
        }),
        "weightoffset" => Some(match effect_direction(effect.change) {
            Some(true) => "overweight",
            Some(false) => "underweight",
            None => "overweight",
        }),
        "happiness" => Some(match effect_direction(effect.change) {
            Some(true) => "happy",
            Some(false) => "miserable",
            None => "happy",
        }),
        _ => None,
    };
    preferred
        .and_then(|base| {
            statuses.iter().find_map(|status| {
                (status_base(status).eq_ignore_ascii_case(base)).then_some(status.as_str())
            })
        })
        .or_else(|| statuses.first().map(String::as_str))
}

fn status_base(status: &str) -> &str {
    status.trim_end_matches(|character: char| character.is_ascii_digit())
}

fn effect_direction(change: Change) -> Option<bool> {
    match change {
        Change::Add(value) if value != 0.0 => Some(value > 0.0),
        Change::Multiply(value) if value != 1.0 => Some(value > 1.0),
        _ => None,
    }
}

fn temperature_direction(change: Change) -> Option<bool> {
    match change {
        Change::Add(value) if value != 0.0 => Some(value > 0.0),
        Change::Multiply(value) if value != 1.0 => Some(value > 1.0),
        Change::Set(value) if value != 37.0 => Some(value > 37.0),
        _ => None,
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
        return (names.field(field), Some(field));
    }
    let label = match (&effect.target, stat) {
        (Target::Component(c), "opiateAmount") if c == "Painkillers" => "Opioids",
        (Target::Component(c), "antagonistAmount") if c == "Painkillers" => "Opioid antagonist",
        (Target::Component(c), "opiateTolerance") if c == "Painkillers" => "Opioid tolerance",
        (Target::Component(c), "amount") if c == "SleepingPills" => "Sleeping pills",
        (_, "disinfect") => "Disinfection",
        (_, "vomit") => "Vomiting",
        _ => return (names.stat(stat), None),
    };
    (names.text(label).into_owned(), None)
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
        Change::Add(v) if v != 0.0 => Some(v > 0.0),
        Change::Multiply(v) if v != 1.0 => Some(v > 1.0),
        _ => None,
    };
    let direction = field.map(|field| {
        let owner = if effect.target == Target::Body {
            "Body"
        } else {
            "Limb"
        };
        crate::game_data::runtime::metadata()
            .moodles
            .directions
            .get(&format!("{owner}.{}", field.key))
            .copied()
            .unwrap_or(match field.good {
                Good::Near(target) if target == field.min => Good::Low,
                Good::Near(target) if target == field.max => Good::High,
                Good::High | Good::Low => field.good,
                _ => Good::Neutral,
            })
    });
    let mut tone = match (direction, rising) {
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
