//! Raw data view: the whole save as a collapsible, editable JSON tree, plus
//! a pretty-printed text editor.

use egui::{Color32, RichText, Ui};
use serde_json::{Map, Number, Value};

use crate::json::{self, PathSeg};

/// Persistent state of the raw view.
#[derive(Default)]
pub struct TreeState {
    /// Case-insensitive filter on keys and scalar values.
    pub search: String,
    /// One-shot "expand all" (`Some(true)`) / "collapse all" (`Some(false)`).
    pub open_override: Option<bool>,
    /// Show the text editor instead of the tree.
    pub text_mode: bool,
    /// Text being edited in text mode.
    pub text: String,
    /// Whether [`TreeState::text`] has edits not yet applied.
    pub text_dirty: bool,
    /// Last parse error of the text editor.
    pub text_error: Option<String>,
}

/// Action requested from a node's context menu.
enum NodeAction {
    Delete,
}

/// Colour of a JSON value type.
fn type_color(v: &Value) -> Color32 {
    match v {
        Value::Null => Color32::GRAY,
        Value::Bool(_) => Color32::from_rgb(200, 120, 220),
        Value::Number(_) => Color32::from_rgb(110, 190, 250),
        Value::String(_) => Color32::from_rgb(220, 180, 100),
        Value::Array(_) | Value::Object(_) => Color32::from_rgb(150, 150, 150),
    }
}

/// Short one-line preview of a value.
fn preview(v: &Value) -> String {
    match v {
        Value::Array(a) => format!("[{} items]", a.len()),
        Value::Object(o) => format!("{{{} fields}}", o.len()),
        Value::String(s) => format!("\"{s}\""),
        other => other.to_string(),
    }
}

/// Whether the value or any descendant matches the lower-cased needle.
fn matches(key: &str, v: &Value, needle: &str) -> bool {
    if needle.is_empty() || key.to_lowercase().contains(needle) {
        return true;
    }
    match v {
        Value::Object(o) => o.iter().any(|(k, c)| matches(k, c, needle)),
        Value::Array(a) => a
            .iter()
            .enumerate()
            .any(|(i, c)| matches(&i.to_string(), c, needle)),
        Value::String(s) => s.to_lowercase().contains(needle),
        other => other.to_string().to_lowercase().contains(needle),
    }
}

/// Inline editor for a scalar value. Returns `true` if changed.
///
/// Numbers keep their integer/float nature, strings get a text field,
/// booleans a checkbox. A small menu allows changing the type.
pub fn leaf_editor(ui: &mut Ui, value: &mut Value) -> bool {
    let mut changed = false;
    match value {
        Value::Bool(b) => changed |= ui.checkbox(b, "").changed(),
        Value::Number(_) => changed |= super::number_editor(ui, value, 0.1),
        Value::String(s) => {
            changed |= ui
                .add(egui::TextEdit::singleline(s).desired_width(220.0))
                .changed()
        }
        Value::Null => {
            ui.label(RichText::new("null").italics().color(Color32::GRAY));
        }
        Value::Array(_) | Value::Object(_) => {
            ui.weak(preview(value));
        }
    }
    if !value.is_array() && !value.is_object() {
        ui.menu_button(RichText::new("⚙").small(), |ui| {
            ui.label(RichText::new("Change type").strong());
            let as_text = match &*value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let mut set = |ui: &mut Ui, label: &str, new: Value| {
                if ui.button(label).clicked() {
                    *value = new;
                    changed = true;
                    ui.close();
                }
            };
            let num = as_text.trim().parse::<f64>().ok();
            set(ui, "null", Value::Null);
            set(ui, "boolean", Value::Bool(as_text == "true"));
            set(
                ui,
                "integer",
                Value::Number(Number::from(num.map_or(0, |n| n.round() as i64))),
            );
            set(
                ui,
                "float",
                Value::Number(json::f32_number(num.unwrap_or(0.0))),
            );
            set(ui, "string", Value::String(as_text.clone()));
        });
    }
    changed
}

/// Shows a JSON value as a collapsible tree rooted at `key`.
///
/// `path` is the path of `value` within the document (used for the context
/// menu's "Copy path"). Returns `true` if anything changed.
pub fn tree(
    ui: &mut Ui,
    key: &str,
    value: &mut Value,
    state: &TreeState,
    path: &mut Vec<PathSeg>,
) -> bool {
    let needle = state.search.to_lowercase();
    let mut changed = false;
    node(ui, key, value, state, &needle, path, &mut changed);
    changed
}

/// Shows a sub-tree with default state (used for unknown nested fields).
pub fn subtree(ui: &mut Ui, id: &str, key: &str, value: &mut Value) -> bool {
    let state = TreeState::default();
    let mut changed = false;
    ui.push_id(id, |ui| {
        node(ui, key, value, &state, "", &mut Vec::new(), &mut changed);
    });
    changed
}

fn context_menu(
    ui: &mut Ui,
    path: &[PathSeg],
    value: &Value,
    deletable: bool,
) -> Option<NodeAction> {
    let mut action = None;
    if ui.button("📋 Copy path").clicked() {
        ui.ctx().copy_text(json::path_to_string(path));
        ui.close();
    }
    if ui.button("📋 Copy value as JSON").clicked() {
        ui.ctx()
            .copy_text(serde_json::to_string_pretty(value).unwrap_or_default());
        ui.close();
    }
    if deletable {
        ui.separator();
        if ui
            .button(RichText::new("🗑 Delete").color(super::colors::BAD))
            .clicked()
        {
            action = Some(NodeAction::Delete);
            ui.close();
        }
    }
    action
}

fn node(
    ui: &mut Ui,
    key: &str,
    value: &mut Value,
    state: &TreeState,
    needle: &str,
    path: &mut Vec<PathSeg>,
    changed: &mut bool,
) -> Option<NodeAction> {
    let deletable = !path.is_empty();
    let mut action = None;
    match value {
        Value::Object(_) | Value::Array(_) => {
            let summary = preview(value);
            let mut job = egui::text::LayoutJob::default();
            let font = egui::FontSelection::Default;
            RichText::new(format!("{key}  ")).strong().append_to(
                &mut job,
                ui.style(),
                font.clone(),
                egui::Align::Center,
            );
            RichText::new(&summary).weak().append_to(
                &mut job,
                ui.style(),
                font,
                egui::Align::Center,
            );
            let id = ui.make_persistent_id(json::path_to_string(path));
            let mut ch = egui::CollapsingHeader::new(egui::WidgetText::from(job))
                .id_salt(id)
                .default_open(path.is_empty());
            if let Some(open) = state.open_override {
                ch = ch.open(Some(open));
            }
            if !needle.is_empty() {
                ch = ch.open(Some(true));
            }
            let resp = ch.show(ui, |ui| match value {
                Value::Object(map) => *changed |= object_children(ui, map, state, needle, path),
                Value::Array(arr) => *changed |= array_children(ui, arr, state, needle, path),
                _ => {}
            });
            let snapshot = value.clone();
            resp.header_response
                .on_hover_text(format!("{}  {summary}", json::path_to_string(path)))
                .context_menu(|ui| action = context_menu(ui, path, &snapshot, deletable));
        }
        _ => {
            ui.horizontal(|ui| {
                let r = ui.label(RichText::new(key).monospace());
                ui.label(":");
                ui.scope(|ui| {
                    ui.visuals_mut().override_text_color = Some(type_color(value));
                    *changed |= leaf_editor(ui, value);
                });
                let snapshot = value.clone();
                r.on_hover_text(json::path_to_string(path))
                    .context_menu(|ui| action = context_menu(ui, path, &snapshot, deletable));
            });
        }
    }
    action
}

fn object_children(
    ui: &mut Ui,
    map: &mut Map<String, Value>,
    state: &TreeState,
    needle: &str,
    path: &mut Vec<PathSeg>,
) -> bool {
    let mut changed = false;
    let mut delete = None;
    let keys: Vec<String> = map.keys().cloned().collect();
    for k in keys {
        let Some(child) = map.get_mut(&k) else {
            continue;
        };
        if !matches(&k, child, needle) {
            continue;
        }
        path.push(PathSeg::Key(k.clone()));
        if let Some(NodeAction::Delete) = node(ui, &k, child, state, needle, path, &mut changed) {
            delete = Some(k.clone());
        }
        path.pop();
    }
    if let Some(k) = delete {
        map.shift_remove(&k);
        changed = true;
    }
    changed
}

fn array_children(
    ui: &mut Ui,
    arr: &mut Vec<Value>,
    state: &TreeState,
    needle: &str,
    path: &mut Vec<PathSeg>,
) -> bool {
    let mut changed = false;
    let mut delete = None;
    for (i, child) in arr.iter_mut().enumerate() {
        let label = format!("[{i}]");
        if !matches(&label, child, needle) {
            continue;
        }
        path.push(PathSeg::Index(i));
        if let Some(NodeAction::Delete) = node(ui, &label, child, state, needle, path, &mut changed)
        {
            delete = Some(i);
        }
        path.pop();
    }
    if let Some(i) = delete {
        arr.remove(i);
        changed = true;
    }
    changed
}

/// Full raw-data tab: toolbar, tree or text editor. Returns `true` if the
/// document changed.
pub fn show(ui: &mut Ui, data: &mut Value, state: &mut TreeState) -> bool {
    let mut changed = false;
    state.open_override = None;
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.text_mode, false, "🌳 Tree");
        if ui
            .selectable_value(&mut state.text_mode, true, "📝 Text")
            .clicked()
            || (state.text_mode && !state.text_dirty && state.text.is_empty())
        {
            state.text = crate::save::to_pretty_json(data);
            state.text_dirty = false;
            state.text_error = None;
        }
        ui.separator();
        if !state.text_mode {
            ui.label("🔍");
            ui.add(
                egui::TextEdit::singleline(&mut state.search)
                    .hint_text("Filter keys / values")
                    .desired_width(220.0),
            );
            if !state.search.is_empty() && ui.small_button("✖").clicked() {
                state.search.clear();
            }
            if ui.button("⊞ Expand all").clicked() {
                state.open_override = Some(true);
            }
            if ui.button("⊟ Collapse all").clicked() {
                state.open_override = Some(false);
            }
        } else {
            if ui
                .add_enabled(state.text_dirty, egui::Button::new("✔ Apply"))
                .clicked()
            {
                match serde_json::from_str::<Value>(&state.text) {
                    Ok(v) => match crate::save::validate(&v) {
                        Ok(()) => {
                            *data = v;
                            changed = true;
                            state.text_dirty = false;
                            state.text_error = None;
                        }
                        Err(e) => state.text_error = Some(e.to_string()),
                    },
                    Err(e) => state.text_error = Some(format!("JSON error: {e}")),
                }
            }
            if ui.button("↺ Reload from data").clicked() {
                state.text = crate::save::to_pretty_json(data);
                state.text_dirty = false;
                state.text_error = None;
            }
            if state.text_dirty {
                ui.colored_label(super::colors::WARN, "Unapplied text changes");
            }
        }
    });
    if let Some(err) = &state.text_error {
        ui.colored_label(super::colors::BAD, err);
    }
    ui.separator();
    if state.text_mode {
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            let r = ui.add(
                egui::TextEdit::multiline(&mut state.text)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(40),
            );
            if r.changed() {
                state.text_dirty = true;
            }
        });
    } else {
        if !state.text_dirty && !state.text.is_empty() {
            // Keep the text view in sync next time it is opened.
            state.text.clear();
        }
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                changed |= tree(ui, "save", data, state, &mut Vec::new());
            });
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn search_matches_nested_values() {
        let v = json!({"items": [{"id": "bandage"}]});
        assert!(matches("root", &v, "band"));
        assert!(!matches("root", &v, "rifle"));
        assert!(matches("root", &v, ""));
    }
}
