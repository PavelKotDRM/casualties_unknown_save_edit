//! Body wiki entries.

use super::*;

pub(super) fn field_entry(names: Names, field: &FieldSpec, group: &str) -> Entry {
    let mut entry = Entry::new(names.field(field));
    entry.subtitle = format!("{} · {}", names.text(group), field.key);
    entry.text = (!field.help.is_empty()).then(|| names.text(field.help).into_owned());
    let unit = if field.unit.is_empty() {
        String::new()
    } else {
        format!(" {}", field.unit)
    };
    match field.kind {
        Kind::Flag => entry.facts.push((
            names.text("Type").into_owned(),
            names.text("on/off").into_owned(),
        )),
        Kind::Int if !field.min.is_finite() => entry.facts.push((
            names.text("Type").into_owned(),
            names.text("whole number").into_owned(),
        )),
        _ if field.min.is_finite() && field.max.is_finite() => entry.facts.push((
            names.text("Range").into_owned(),
            format!(
                "{} – {}{unit}",
                fmt_num(f64::from(field.min)),
                fmt_num(f64::from(field.max))
            ),
        )),
        _ => entry.facts.push((
            names.text("Type").into_owned(),
            format!("{}{unit}", names.text("number")),
        )),
    }
    let better = match field.good {
        Good::High => Some(names.text("higher is better").into_owned()),
        Good::Low => Some(names.text("lower is better").into_owned()),
        Good::Near(target) => Some(format!(
            "{} {}{unit}",
            names.text("best near"),
            fmt_num(f64::from(target))
        )),
        Good::Neutral => None,
    };
    if let Some(better) = better {
        entry
            .facts
            .push((names.text("Healthy").into_owned(), better));
    }
    entry
}

pub(super) fn limbs(names: Names) -> Vec<Entry> {
    let mut list_entry = Entry::new(names.text("Limb list"));
    list_entry.text = Some(
        names
            .text("Order in which the save stores limbs (body.limbs).")
            .into_owned(),
    );
    list_entry.facts = schema::LIMBS
        .iter()
        .enumerate()
        .map(|(index, def)| {
            (
                format!("#{index}"),
                format!("{} ({})", names.limb(index), def.object),
            )
        })
        .collect();
    std::iter::once(list_entry)
        .chain(
            schema::LIMB_FIELDS
                .iter()
                .map(|field| field_entry(names, field, "Limb")),
        )
        .collect()
}
