//! Shared egui widgets for editing and displaying save fields.

use super::text::fmt_num;
use super::value_color;
use super::{LABEL_WIDTH, Names, colors, raw};
use crate::json::{self, is_integer};
use crate::schema::{FieldSpec, Good, Kind};
use egui::{
    Align2, Color32, CornerRadius, FontId, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use serde_json::{Map, Value};

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
pub fn mini_gauge(ui: &mut Ui, spec: &FieldSpec, value: f64, width: f32, names: Names) -> Response {
    let height = 8.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
    let t = ((value as f32 - spec.min) / (spec.max - spec.min).max(f32::EPSILON)).clamp(0.0, 1.0);
    let fill = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * t, height));
    painter.rect_filled(fill, 3.0, value_color(spec, value as f32));
    response.on_hover_text(format!(
        "{}: {}{}\n{}",
        names.field(spec),
        fmt_num(value),
        spec.unit,
        names.text("JSON key: {key}").replace("{key}", spec.key),
    ))
}

/// Label cell of a field row, with tooltip and the JSON key on hover.
fn field_label(ui: &mut Ui, spec: &FieldSpec, names: Names) {
    ui.add_sized(
        [LABEL_WIDTH, ui.spacing().interact_size.y],
        egui::Label::new(names.field(spec)).truncate(),
    )
    .on_hover_ui(|ui| {
        ui.strong(names.field(spec));
        if !spec.help.is_empty() {
            ui.label(names.text(spec.help).to_string());
        }
        ui.weak(names.text("JSON key: {key}").replace("{key}", spec.key));
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

/// A resizable window that never grows past the main window; its content
/// should use a [`egui::ScrollArea`] for anything taller than the screen.
pub fn bounded_window<'open>(
    ctx: &egui::Context,
    title: impl Into<egui::WidgetText>,
    default_size: Vec2,
    min_width: f32,
) -> egui::Window<'open> {
    let screen = ctx.content_rect();
    let max_size = (screen.size() - Vec2::splat(32.0)).max(Vec2::new(240.0, 200.0));
    egui::Window::new(title)
        .constrain_to(screen)
        .resizable(true)
        .default_size(default_size.min(max_size))
        .min_width(min_width.min(max_size.x))
        .min_height(240.0f32.min(max_size.y))
        .max_size(max_size)
}
