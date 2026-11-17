//! Run wiki entries.

use super::*;

pub(in crate::ui::wiki) fn run_settings(names: Names) -> Vec<Entry> {
    let defaults = schema::run_settings_for_preset("normal").unwrap_or_default();
    schema::RUN_SETTINGS
        .iter()
        .map(|setting| {
            let mut entry = Entry::new(names.run_setting(setting.id));
            entry.subtitle = format!("{} · {}", names.text(setting.section), setting.id);
            entry.text = names.run_setting_description(setting.id);
            match setting.kind {
                RunSettingKind::Bool => {}
                RunSettingKind::Float { min, max, whole } => entry.facts.push((
                    names.text("Range").into_owned(),
                    format!(
                        "{} – {}{}{}",
                        fmt_num(f64::from(min)),
                        fmt_num(f64::from(max)),
                        setting.postfix,
                        if whole {
                            format!(" ({})", names.text("whole numbers"))
                        } else {
                            String::new()
                        }
                    ),
                )),
                RunSettingKind::Dropdown(options) => entry.facts.push((
                    names.text("Options").into_owned(),
                    (0..options.len())
                        .map(|i| names.run_setting_option(setting.id, i))
                        .collect::<Vec<_>>()
                        .join(", "),
                )),
            }
            let default = defaults
                .iter()
                .find(|v| v.get("Item1").and_then(Value::as_str) == Some(setting.id))
                .and_then(|v| v.get("Item2"));
            if let Some(default) = default {
                entry.facts.push((
                    names.text("Default").into_owned(),
                    format_setting(names, setting.id, default),
                ));
            }
            entry
        })
        .collect()
}

/// Formats a run-setting value the way the game's settings screen shows it.
pub(super) fn format_setting(names: Names, id: &str, value: &Value) -> String {
    let def = schema::run_setting(id);
    match (value, def.map(|d| d.kind)) {
        (Value::Bool(on), _) => names.text(if *on { "On" } else { "Off" }).into_owned(),
        (Value::Number(n), Some(RunSettingKind::Dropdown(_))) => {
            names.run_setting_option(id, n.as_u64().unwrap_or(0) as usize)
        }
        (Value::Number(n), _) => format!(
            "{}{}",
            fmt_num(n.as_f64().unwrap_or(0.0)),
            def.map_or("", |d| d.postfix)
        ),
        _ => value.to_string(),
    }
}

pub(super) fn presets(names: Names) -> Vec<Entry> {
    schema::RUN_PRESETS
        .iter()
        .map(|preset| {
            let locale = |key: String| {
                names
                    .locale
                    .and_then(|l| l.other(&key))
                    .map(strip_rich_text)
            };
            let mut entry = Entry::new(
                locale(format!("preset{}", preset.id)).unwrap_or_else(|| preset.name.to_owned()),
            );
            entry.subtitle = preset.id.to_owned();
            entry.text = locale(format!("preset{}dsc", preset.id));
            let values = schema::run_settings_for_preset(preset.id).unwrap_or_default();
            entry.facts = preset
                .overrides
                .iter()
                .filter_map(|(id, _)| {
                    let value = values
                        .iter()
                        .find(|v| v.get("Item1").and_then(Value::as_str) == Some(id))?
                        .get("Item2")?;
                    Some((names.run_setting(id), format_setting(names, id, value)))
                })
                .collect();
            if preset.overrides.is_empty() {
                entry.facts.push((
                    names.text("Settings").into_owned(),
                    names
                        .text("standard values (see World settings)")
                        .into_owned(),
                ));
            }
            entry
        })
        .collect()
}
