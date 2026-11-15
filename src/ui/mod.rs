//! egui views of the save editor.
//!
//! Each tab of the application lives in its own module. This module contains
//! widgets shared between them: colour-coded gauges similar to the in-game
//! health screen, field editors for JSON values and display-name lookup.

pub mod character;
pub mod health;
mod interface_language;
pub mod inventory;
pub use interface_language::InterfaceLanguage;
pub mod limbs;
pub mod overview;
pub mod raw;
pub mod run;

use egui::{
    Align2, Color32, CornerRadius, FontId, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use serde_json::{Map, Value};
use std::borrow::Cow;

use crate::json::{self, is_integer};
use crate::locale::Locale;
use crate::schema::{self, FieldSpec, Good, Kind};

/// Width of the label column in field rows.
pub const LABEL_WIDTH: f32 = 190.0;

/// Colour palette.
pub mod colors {
    use egui::Color32;
    /// Healthy value.
    pub const GOOD: Color32 = Color32::from_rgb(76, 175, 80);
    /// Borderline value.
    pub const WARN: Color32 = Color32::from_rgb(230, 180, 40);
    /// Dangerous value.
    pub const BAD: Color32 = Color32::from_rgb(220, 60, 50);
    /// Value without judgement.
    pub const NEUTRAL: Color32 = Color32::from_rgb(70, 130, 200);
    /// Accent for headers.
    pub const ACCENT: Color32 = Color32::from_rgb(255, 170, 60);
    /// Blood red.
    pub const BLOOD: Color32 = Color32::from_rgb(200, 20, 30);
    /// Infection green.
    pub const INFECTION: Color32 = Color32::from_rgb(120, 200, 60);
}

/// Interpolates red → yellow → green for a quality in `0..=1`.
pub fn quality_color(q: f32) -> Color32 {
    let q = q.clamp(0.0, 1.0);
    let lerp = |a: Color32, b: Color32, t: f32| {
        let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
        Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
    };
    if q < 0.5 {
        lerp(colors::BAD, colors::WARN, q * 2.0)
    } else {
        lerp(colors::WARN, colors::GOOD, (q - 0.5) * 2.0)
    }
}

/// Quality (`0` = bad, `1` = good) of a value according to its spec, or
/// `None` for neutral fields.
pub fn quality(spec: &FieldSpec, value: f32) -> Option<f32> {
    let span = (spec.max - spec.min).max(f32::EPSILON);
    let t = ((value - spec.min) / span).clamp(0.0, 1.0);
    match spec.good {
        Good::High => Some(t),
        Good::Low => Some(1.0 - t),
        Good::Near(target) => {
            let half = (target - spec.min).max(spec.max - target).max(f32::EPSILON);
            Some((1.0 - (value - target).abs() / half * 1.6).clamp(0.0, 1.0))
        }
        Good::Neutral => None,
    }
}

/// Colour for a value according to its spec.
pub fn value_color(spec: &FieldSpec, value: f32) -> Color32 {
    quality(spec, value).map_or(colors::NEUTRAL, quality_color)
}

/// Formats a number compactly (no trailing zeros, at most 2 decimals).
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e9 {
        format!("{v:.0}")
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// Paints an interactive horizontal gauge. Clicking or dragging on the bar
/// sets the value. Returns `true` if the value changed.
pub fn gauge_bar(ui: &mut Ui, spec: &FieldSpec, value: &mut f64, width: f32) -> bool {
    let height = ui.spacing().interact_size.y;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
    let (min, max) = (spec.min as f64, spec.max as f64);
    let mut changed = false;
    if let Some(pos) = response.interact_pointer_pos()
        && (response.is_pointer_button_down_on() || response.clicked())
    {
        let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
        let mut new = min + t * (max - min);
        if spec.kind == Kind::Int {
            new = new.round();
        } else if max - min >= 20.0 {
            new = (new * 10.0).round() / 10.0;
        } else {
            new = (new * 1000.0).round() / 1000.0;
        }
        if (new - *value).abs() > f64::EPSILON {
            *value = new;
            changed = true;
        }
    }
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let visuals = ui.visuals();
        let radius = CornerRadius::same(4);
        painter.rect_filled(rect, radius, visuals.extreme_bg_color);
        let span = (max - min).max(f64::EPSILON);
        let x_of =
            |v: f64| rect.left() + (((v - min) / span).clamp(0.0, 1.0) as f32) * rect.width();
        // Bars of ranges spanning zero grow from zero, like a balance meter.
        let origin = if min < 0.0 && max > 0.0 { 0.0 } else { min };
        let (x0, x1) = (x_of(origin), x_of(*value));
        let fill = egui::Rect::from_x_y_ranges(x0.min(x1)..=x0.max(x1), rect.y_range());
        painter.rect_filled(
            fill,
            radius,
            value_color(spec, *value as f32).gamma_multiply(0.85),
        );
        if origin != min {
            painter.line_segment(
                [egui::pos2(x0, rect.top()), egui::pos2(x0, rect.bottom())],
                Stroke::new(1.0, visuals.weak_text_color()),
            );
        }
        if let Good::Near(target) = spec.good {
            let x = x_of(target as f64);
            painter.line_segment(
                [
                    egui::pos2(x, rect.top() + 2.0),
                    egui::pos2(x, rect.bottom() - 2.0),
                ],
                Stroke::new(1.5, Color32::from_white_alpha(140)),
            );
        }
        let stroke = if response.hovered() {
            visuals.widgets.hovered.bg_stroke
        } else {
            visuals.widgets.inactive.bg_stroke
        };
        painter.rect_stroke(rect, radius, stroke, StrokeKind::Inside);
        let text = format!("{}{}", fmt_num(*value), spec.unit);
        let font = FontId::proportional(12.5);
        for (offset, color) in [
            (Vec2::splat(1.0), Color32::from_black_alpha(200)),
            (Vec2::ZERO, Color32::WHITE),
        ] {
            painter.text(
                rect.center() + offset,
                Align2::CENTER_CENTER,
                &text,
                font.clone(),
                color,
            );
        }
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Click or drag to set the value");
    changed
}

/// Read-only small gauge (used in summaries).
pub fn mini_gauge(ui: &mut Ui, spec: &FieldSpec, value: f64, width: f32) -> Response {
    let height = 8.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
    let t = ((value as f32 - spec.min) / (spec.max - spec.min).max(f32::EPSILON)).clamp(0.0, 1.0);
    let fill = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * t, height));
    painter.rect_filled(fill, 3.0, value_color(spec, value as f32));
    response.on_hover_text(format!("{}: {}{}", spec.label, fmt_num(value), spec.unit))
}

/// Label cell of a field row, with tooltip and the JSON key on hover.
fn field_label(ui: &mut Ui, spec: &FieldSpec, names: Names) {
    ui.add_sized(
        [LABEL_WIDTH, ui.spacing().interact_size.y],
        egui::Label::new(names.text(spec.label).to_string()).truncate(),
    )
    .on_hover_ui(|ui| {
        ui.strong(names.text(spec.label).to_string());
        if !spec.help.is_empty() {
            ui.label(names.text(spec.help).to_string());
        }
        ui.weak(format!("JSON key: {}", spec.key));
    });
}

/// Edits a numeric JSON value in place. Returns `true` if changed.
pub fn number_editor(ui: &mut Ui, slot: &mut Value, speed: f64) -> bool {
    let int = matches!(slot, Value::Number(n) if is_integer(n));
    if int {
        let mut v = slot.as_i64().unwrap_or(0);
        let changed = ui
            .add(egui::DragValue::new(&mut v).speed(speed.max(0.1)))
            .changed();
        if changed {
            json::set_number_preserving(slot, v as f64);
        }
        changed
    } else {
        let mut v = slot.as_f64().unwrap_or(0.0);
        let changed = ui
            .add(egui::DragValue::new(&mut v).speed(speed).max_decimals(4))
            .changed();
        if changed {
            json::set_number_preserving(slot, v);
        }
        changed
    }
}

/// Renders one row for a known field of `obj` (label + editor).
///
/// Missing fields are shown as disabled. Returns `true` if the value changed.
pub fn field_row(
    ui: &mut Ui,
    spec: &FieldSpec,
    obj: &mut Map<String, Value>,
    bar_width: f32,
    names: Names,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        field_label(ui, spec, names);
        let Some(slot) = obj.get_mut(spec.key) else {
            ui.weak(names.text("— not present in this save —").to_string());
            return;
        };
        match spec.kind {
            Kind::Flag => {
                let mut b = slot.as_bool().unwrap_or(false);
                let text = if b {
                    RichText::new(names.text("Yes").to_string()).color(colors::ACCENT)
                } else {
                    RichText::new(names.text("No").to_string())
                };
                if ui.checkbox(&mut b, text).changed() {
                    *slot = Value::Bool(b);
                    changed = true;
                }
            }
            Kind::Gauge => {
                let mut v = slot.as_f64().unwrap_or(0.0);
                if gauge_bar(ui, spec, &mut v, bar_width) {
                    json::set_number_preserving(slot, v);
                    changed = true;
                }
                let speed = ((spec.max - spec.min) as f64 / 300.0).max(0.001);
                changed |= number_editor(ui, slot, speed);
            }
            Kind::Number => changed |= number_editor(ui, slot, 0.1),
            Kind::Int => changed |= number_editor(ui, slot, 0.2),
        }
    });
    changed
}

/// Generic row for a field without a spec (label = key).
pub fn generic_row(ui: &mut Ui, key: &str, value: &mut Value) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized(
            [LABEL_WIDTH, ui.spacing().interact_size.y],
            egui::Label::new(RichText::new(key).monospace()).truncate(),
        );
        changed = raw::leaf_editor(ui, value);
    });
    changed
}

/// Shows unknown fields of an object (those for which `known` is false) with
/// generic editors, so data from newer game versions stays editable.
/// Returns `true` if anything changed.
pub fn other_fields(
    ui: &mut Ui,
    id: &str,
    obj: &mut Map<String, Value>,
    known: &dyn Fn(&str) -> bool,
) -> bool {
    let unknown: Vec<String> = obj.keys().filter(|k| !known(k)).cloned().collect();
    if unknown.is_empty() {
        return false;
    }
    let mut changed = false;
    egui::CollapsingHeader::new(format!("Other fields ({})", unknown.len()))
        .id_salt(id)
        .show(ui, |ui| {
            for key in unknown {
                if let Some(v) = obj.get_mut(&key) {
                    changed |= if v.is_object() || v.is_array() {
                        raw::subtree(ui, &format!("{id}/{key}"), &key, v)
                    } else {
                        generic_row(ui, &key, v)
                    };
                }
            }
        });
    changed
}

/// Draws a section heading with an icon.
pub fn section_heading(ui: &mut Ui, names: Names, icon: &str, title: &str) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(format!("{icon}  {}", names.text(title)))
            .heading()
            .color(colors::ACCENT),
    );
    ui.add_space(2.0);
}

/// A framed card.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::group(ui.style())
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::same(10))
        .show(ui, add)
        .inner
}

/// Display-name lookup that prefers the game's language file.
#[derive(Clone, Copy)]
pub struct Names<'a> {
    /// Loaded locale, if the game installation was found.
    pub locale: Option<&'a Locale>,
    /// Language used by the editor's interface.
    pub interface_language: InterfaceLanguage<'a>,
}

impl Names<'_> {
    /// Translates an editor interface string using the selected UI language.
    pub fn text<'a>(&self, source: &'a str) -> Cow<'a, str> {
        self.interface_language.text(source)
    }

    /// Item display name (falls back to the id).
    pub fn item(&self, id: &str) -> String {
        self.locale
            .and_then(|l| l.item_name(id))
            .map_or_else(|| prettify_id(id), str::to_owned)
    }

    /// Item description, if available.
    pub fn item_description(&self, id: &str) -> Option<String> {
        self.locale
            .and_then(|l| l.item_description(id))
            .map(strip_rich_text)
    }

    /// Liquid display name.
    pub fn liquid(&self, id: &str) -> String {
        self.item(id)
    }

    /// Limb display name by index.
    pub fn limb(&self, index: usize) -> String {
        let Some(def) = schema::LIMBS.get(index) else {
            return format!("Limb #{index}");
        };
        let name = self
            .locale
            .and_then(|l| l.other(def.object))
            .unwrap_or(def.name)
            .to_owned();
        self.text(&name).into_owned()
    }

    /// Run setting label.
    pub fn run_setting(&self, id: &str) -> String {
        self.locale
            .and_then(|l| l.other(&format!("runset{id}")))
            .map(str::to_owned)
            .or_else(|| schema::run_setting(id).map(|d| d.label.to_owned()))
            .unwrap_or_else(|| id.to_owned())
    }

    /// Run setting description.
    pub fn run_setting_description(&self, id: &str) -> Option<String> {
        self.locale
            .and_then(|l| l.other(&format!("runset{id}dsc")))
            .map(strip_rich_text)
    }

    /// Dropdown option label of a run setting.
    pub fn run_setting_option(&self, id: &str, index: usize) -> String {
        let fallback = schema::run_setting(id).and_then(|d| match d.kind {
            schema::RunSettingKind::Dropdown(opts) => opts.get(index).copied(),
            _ => None,
        });
        let key = fallback.map(|f| format!("runset{id}{}", f.to_lowercase()));
        key.and_then(|k| self.locale.and_then(|l| l.other(&k)).map(str::to_owned))
            .or_else(|| fallback.map(str::to_owned))
            .unwrap_or_else(|| format!("Option {index}"))
    }

    /// Skill display name.
    pub fn skill(&self, def: &schema::SkillDef) -> String {
        let name = self
            .locale
            .and_then(|l| l.other(def.locale_key))
            .unwrap_or(def.name)
            .to_owned();
        self.text(&name).into_owned()
    }

    /// Name of the layer the run resumes on (`biome` field).
    pub fn layer(&self, biome: i64) -> String {
        let from_locale = biome
            .checked_add(1)
            .and_then(|layer| {
                self.locale
                    .and_then(|locale| locale.other(&format!("layertitle{layer}")))
            })
            .map(|s| s.replace('\n', " — "));
        from_locale
            .or_else(|| {
                usize::try_from(biome)
                    .ok()
                    .and_then(|i| schema::LAYERS.get(i))
                    .map(|s| (*s).to_owned())
            })
            .unwrap_or_else(|| self.text("Unknown layer").into_owned())
    }
}

/// Turns `sterilizedbandage` into `Sterilizedbandage` as a last-resort name.
pub fn prettify_id(id: &str) -> String {
    let mut c = id.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => "(empty id)".to_owned(),
    }
}

/// Removes Unity rich-text tags (`<color=..>`, `<b>`, `<sprite ..>`).
pub fn strip_rich_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0;
    for ch in s.chars() {
        match ch {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Icon for an item category.
pub fn category_icon(category: &str) -> &'static str {
    match category {
        "medical" => "✚",
        "drug" => "💊",
        "food" => "🍞",
        "water" => "💧",
        "container" => "🎒",
        "tool" => "🔧",
        "utility" => "🔦",
        "trash" => "🗑",
        "custom" => "★",
        "unobtainable" => "⛔",
        _ => "📦",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_follows_good_direction() {
        let pain = schema::limb_field("pain").unwrap();
        assert_eq!(quality(pain, 0.0), Some(1.0));
        assert_eq!(quality(pain, 100.0), Some(0.0));
        let hr = schema::body_field("heartRate").unwrap();
        assert!(quality(hr, 70.0).unwrap() > quality(hr, 180.0).unwrap());
    }

    #[test]
    fn rich_text_is_stripped() {
        assert_eq!(
            strip_rich_text("<color=#fff>Hot</color> <sprite index=1>tea"),
            "Hot tea"
        );
    }

    #[test]
    fn russian_interface_translates_inventory_and_encumbrance_labels() {
        assert_eq!(InterfaceLanguage::Russian.text("Inventory"), "Инвентарь");
        assert_eq!(
            InterfaceLanguage::Russian.text("⚠ Character is overburdened."),
            "⚠ Персонаж перегружен."
        );
    }

    #[test]
    fn layer_lookup_handles_maximum_saved_index() {
        let locale = Locale::from_json(r#"{"name":"English","other":{}}"#).unwrap();
        let names = Names {
            locale: Some(&locale),
            interface_language: InterfaceLanguage::English,
        };
        assert_eq!(names.layer(i64::MAX), "Unknown layer");
    }
}
