//! Scaled body-figure geometry, painting, and legend.

use super::super::{Names, colors, fmt_num, quality_color, value_color};
use super::geometry::{FIGURE, limb_rect};
use super::{limb_condition, limb_problems};
use crate::json::{get_bool, get_f64};
use crate::schema;
use egui::{Color32, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use serde_json::Value;

/// Paints the body figure scaled to `height`. Returns the index of a
/// clicked limb.
pub fn body_figure(
    ui: &mut Ui,
    limbs: &[Value],
    names: Names,
    selected: Option<usize>,
    height: f32,
) -> Option<usize> {
    let scale = height / 400.0;
    let (rect, response) = ui.allocate_exact_size(vec2(200.0 * scale, height), Sense::click());
    let hover = response.hover_pos();
    let mut hovered = None;
    let painter = ui.painter_at(rect.expand(4.0));
    let outline = ui.visuals().strong_text_color();
    for (i, &shape) in FIGURE.iter().enumerate() {
        let r = limb_rect(rect, shape, scale);
        let limb = limbs.get(i).unwrap_or(&Value::Null);
        let obj = limb.as_object();
        let flag = |k| obj.and_then(|o| get_bool(o, k)).unwrap_or(false);
        let num = |k| obj.and_then(|o| get_f64(o, k)).unwrap_or(0.0);
        let radius = if i == 0 {
            r.width() * 0.45
        } else {
            5.0 * scale
        };
        if hover.is_some_and(|p| r.contains(p)) {
            hovered = Some(i);
        }
        if flag("dismembered") {
            painter.rect_stroke(
                r,
                radius,
                Stroke::new(1.0, Color32::from_gray(110)),
                StrokeKind::Inside,
            );
        } else {
            painter.rect_filled(
                r,
                radius,
                quality_color(limb_condition(limb)).gamma_multiply(0.9),
            );
            let infection = num("infectionAmount");
            if infection > 0.0 || flag("infected") {
                let t = (infection / 100.0).clamp(0.3, 1.0) as f32;
                painter.rect_filled(
                    r.shrink(r.width().min(r.height()) * 0.25),
                    radius,
                    colors::INFECTION.gamma_multiply(t * 0.8),
                );
            }
            if flag("broken") {
                let c = r.center();
                let half = vec2(r.width() * 0.32, r.height().min(r.width()) * 0.18);
                fracture_mark(&painter, Rect::from_center_size(c, half * 2.0));
            }
            if flag("dislocated") {
                painter.circle_stroke(
                    pos2(r.center().x, r.top() + 4.0 * scale),
                    4.0 * scale,
                    Stroke::new(2.0, colors::WARN),
                );
            }
            if flag("blockedBleeding") {
                let y = r.top() + r.height() * 0.2;
                painter.line_segment(
                    [pos2(r.left(), y), pos2(r.right(), y)],
                    Stroke::new(3.0 * scale, Color32::BLACK),
                );
            }
            if flag("splinted") {
                painter.rect_stroke(
                    r.shrink(2.0),
                    radius,
                    Stroke::new(1.5, Color32::from_gray(230)),
                    StrokeKind::Inside,
                );
            }
            let bleed = num("bleedAmount");
            if bleed > 0.0 {
                let size = (2.5 + (bleed as f32 / 100.0) * 4.0) * scale.max(0.6);
                painter.circle_filled(
                    pos2(r.right() - size - 1.0, r.top() + size + 2.0),
                    size,
                    colors::BLOOD,
                );
            }
            if obj
                .and_then(|o| o.get("prostheticInfo"))
                .and_then(|p| p.get("hasProsthetic"))
                .and_then(Value::as_bool)
                == Some(true)
            {
                painter.rect_stroke(
                    r,
                    radius,
                    Stroke::new(2.0, Color32::from_rgb(160, 200, 255)),
                    StrokeKind::Outside,
                );
            }
        }
        let stroke = if selected == Some(i) {
            Stroke::new(2.5, colors::ACCENT)
        } else if hovered == Some(i) {
            Stroke::new(1.5, outline)
        } else {
            Stroke::new(1.0, Color32::from_black_alpha(160))
        };
        painter.rect_stroke(r, radius, stroke, StrokeKind::Outside);
    }
    if let Some(i) = hovered {
        let limb = limbs.get(i).unwrap_or(&Value::Null);
        response.clone().on_hover_ui_at_pointer(|ui| {
            ui.strong(names.limb(i));
            if let Some(obj) = limb.as_object() {
                for key in [
                    "skinHealth",
                    "muscleHealth",
                    "pain",
                    "bleedAmount",
                    "infectionAmount",
                ] {
                    if let (Some(spec), Some(v)) = (schema::limb_field(key), get_f64(obj, key)) {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}:", names.field(spec)));
                            ui.colored_label(value_color(spec, v as f32), fmt_num(v));
                        });
                    }
                }
            }
            let problems = limb_problems(limb);
            if !problems.is_empty() {
                ui.colored_label(colors::WARN, problems.join(", "));
            }
        });
    }
    if response.clicked() { hovered } else { None }
}

/// Zig-zag crack drawn on broken limbs.
fn fracture_mark(painter: &egui::Painter, r: Rect) {
    let points = vec![
        r.left_center(),
        pos2(r.left() + r.width() * 0.33, r.top()),
        pos2(r.left() + r.width() * 0.66, r.bottom()),
        r.right_center(),
    ];
    painter.add(egui::Shape::line(
        points.clone(),
        Stroke::new(3.5, Color32::BLACK),
    ));
    painter.add(egui::Shape::line(points, Stroke::new(2.0, Color32::WHITE)));
}

/// Legend explaining the figure's markers.
pub fn legend(ui: &mut Ui, names: Names) {
    let item = |ui: &mut Ui, paint: &dyn Fn(&egui::Painter, Rect), text: &str| {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
            paint(ui.painter(), r);
            ui.small(names.text(text).to_string());
        });
    };
    item(
        ui,
        &|p, r| {
            let (left, right) = r.split_left_right_at_fraction(0.5);
            p.rect_filled(left, 2.0, quality_color(1.0));
            p.rect_filled(right, 2.0, quality_color(0.0));
        },
        "Condition (healthy → injured)",
    );
    item(
        ui,
        &|p, r| {
            p.circle_filled(r.center(), 4.0, colors::BLOOD);
        },
        "Bleeding",
    );
    item(
        ui,
        &|p, r| {
            p.rect_filled(r.shrink(3.0), 2.0, colors::INFECTION);
        },
        "Infection",
    );
    item(ui, &|p, r| fracture_mark(p, r.shrink(1.0)), "Fracture");
    item(
        ui,
        &|p, r| {
            p.circle_stroke(r.center(), 4.0, Stroke::new(2.0, colors::WARN));
        },
        "Dislocation",
    );
    item(
        ui,
        &|p, r| {
            p.line_segment(
                [r.left_center(), r.right_center()],
                Stroke::new(3.0, Color32::BLACK),
            );
        },
        "Tourniquet",
    );
    item(
        ui,
        &|p, r| {
            p.rect_stroke(
                r,
                2.0,
                Stroke::new(1.0, Color32::from_gray(110)),
                StrokeKind::Inside,
            );
        },
        "Dismembered",
    );
    item(
        ui,
        &|p, r| {
            p.rect_stroke(
                r.shrink(2.0),
                2.0,
                Stroke::new(2.0, Color32::from_rgb(160, 200, 255)),
                StrokeKind::Outside,
            );
        },
        "Prosthetic",
    );
}
/// Painted marker used in summaries: the figure without interactions.
pub fn small_figure(ui: &mut Ui, data: &Value, names: Names, height: f32) -> Option<usize> {
    let limbs = data
        .get("limbs")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    body_figure(ui, limbs, names, None, height)
}
