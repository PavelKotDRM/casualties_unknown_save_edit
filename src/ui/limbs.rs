//! Limbs tab: a painted body figure (like the in-game health screen) and an
//! editor for the selected limb.

// FIXME: ~525 lines; `body_figure` is a ~145-line painter routine (layout, hit testing, fill
// colours, fracture/dismemberment marks, tooltips) and `legend` ~75 lines of hand-placed swatches,
// both sitting next to the limb editor/table. Move the figure painting (`FIGURE`, `body_figure`,
// `small_figure`, `fracture_mark`, `legend`) into `ui/limbs/figure.rs` with the geometry
// separated from the painting, and move `limb_condition`/`limb_problems` (pure save-document
// logic that is `pub` but only needed by the UI here) into `model::health` so `diagnostics.rs`
// and the report can share them instead of re-deriving limb state.

use egui::{Color32, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use serde_json::Value;

use super::{Names, colors, field_row, other_fields, quality_color};
use crate::json::{get_bool, get_f64};
use crate::{model, schema};

/// UI state of the limbs tab.
#[derive(Default)]
pub struct LimbsState {
    /// Index of the selected limb.
    pub selected: usize,
}

/// Layout of the body figure in a 200 × 400 design space. The viewer's left
/// is the character's right side (`F` limbs), as on the in-game screen.
const FIGURE: [(f32, f32, f32, f32); 15] = [
    (75.0, 5.0, 125.0, 62.0),     // Head
    (68.0, 68.0, 132.0, 148.0),   // UpTorso
    (72.0, 150.0, 128.0, 214.0),  // DownTorso
    (40.0, 70.0, 64.0, 144.0),    // UpArmF
    (38.0, 146.0, 62.0, 212.0),   // DownArmF
    (34.0, 214.0, 62.0, 244.0),   // HandF
    (136.0, 70.0, 160.0, 144.0),  // UpArmB
    (138.0, 146.0, 162.0, 212.0), // DownArmB
    (138.0, 214.0, 166.0, 244.0), // HandB
    (72.0, 218.0, 98.0, 292.0),   // ThighF
    (74.0, 294.0, 97.0, 362.0),   // CrusF
    (62.0, 364.0, 97.0, 384.0),   // FootF
    (102.0, 218.0, 128.0, 292.0), // ThighB
    (103.0, 294.0, 126.0, 362.0), // CrusB
    (103.0, 364.0, 138.0, 384.0), // FootB
];

/// Overall condition (`0..1`) of a limb: the worse of skin and muscle health.
pub fn limb_condition(limb: &Value) -> f32 {
    let Some(obj) = limb.as_object() else {
        return 0.0;
    };
    let skin = get_f64(obj, "skinHealth").unwrap_or(100.0);
    let muscle = get_f64(obj, "muscleHealth").unwrap_or(100.0);
    (skin.min(muscle) / 100.0).clamp(0.0, 1.0) as f32
}

/// One-line list of the limb's problems (empty if healthy).
pub fn limb_problems(limb: &Value) -> Vec<&'static str> {
    let Some(obj) = limb.as_object() else {
        return Vec::new();
    };
    let flag = |k| get_bool(obj, k).unwrap_or(false);
    let num = |k| get_f64(obj, k).unwrap_or(0.0);
    let mut out = Vec::new();
    if flag("dismembered") {
        out.push("dismembered");
        return out;
    }
    if flag("broken") {
        out.push("broken");
    }
    if flag("dislocated") {
        out.push("dislocated");
    }
    if num("bleedAmount") > 0.0 {
        out.push("bleeding");
    }
    if flag("infected") || num("infectionAmount") > 0.0 {
        out.push("infected");
    }
    if num("pain") > 20.0 {
        out.push("in pain");
    }
    if obj.get("shrapnel").and_then(Value::as_i64).unwrap_or(0) > 0 {
        out.push("shrapnel");
    }
    if flag("blockedBleeding") {
        out.push("tourniquet");
    }
    if flag("splinted") {
        out.push("splinted");
    }
    if obj
        .get("prostheticInfo")
        .and_then(|p| p.get("hasProsthetic"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        out.push("prosthetic");
    }
    out
}

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
    let to_screen = |(x0, y0, x1, y1): (f32, f32, f32, f32)| {
        Rect::from_min_max(
            rect.min + vec2(x0, y0) * scale,
            rect.min + vec2(x1, y1) * scale,
        )
    };
    let hover = response.hover_pos();
    let mut hovered = None;
    let painter = ui.painter_at(rect.expand(4.0));
    let outline = ui.visuals().strong_text_color();
    for (i, &shape) in FIGURE.iter().enumerate() {
        let r = to_screen(shape);
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
                            ui.label(format!("{}:", names.text(spec.label)));
                            ui.colored_label(super::value_color(spec, v as f32), super::fmt_num(v));
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

fn heal_button(ui: &mut Ui, data: &mut Value, label: &str, regrow: bool) -> bool {
    if ui.button(label).clicked() {
        model::heal_limbs(data, regrow);
        return true;
    }
    false
}

/// Shows the limbs tab. Returns `true` if the document changed.
pub fn show(ui: &mut Ui, data: &mut Value, names: Names, state: &mut LimbsState) -> bool {
    let mut changed = false;
    let Some(limbs) = data.get("limbs").and_then(Value::as_array).cloned() else {
        ui.colored_label(
            colors::BAD,
            names.text("This save has no `limbs` array.").to_string(),
        );
        return false;
    };
    state.selected = state.selected.min(limbs.len().saturating_sub(1));
    ui.horizontal(|ui| {
        changed |= heal_button(
            ui,
            data,
            &format!("✚ {}", names.text("Heal all limbs")),
            false,
        );
        changed |= heal_button(
            ui,
            data,
            &format!("✚ {}", names.text("Heal all + regrow dismembered")),
            true,
        );
    });
    ui.separator();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(220.0);
            if let Some(i) = body_figure(ui, &limbs, names, Some(state.selected), 420.0) {
                state.selected = i;
            }
            ui.add_space(6.0);
            legend(ui, names);
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    changed |= limb_editor(ui, data, names, state);
                    ui.add_space(10.0);
                    limb_table(ui, data, names, state);
                });
            });
    });
    changed
}

fn limb_editor(ui: &mut Ui, data: &mut Value, names: Names, state: &mut LimbsState) -> bool {
    let mut changed = false;
    let count = data
        .get("limbs")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("limb_select")
            .selected_text(RichText::new(names.limb(state.selected)).heading())
            .width(220.0)
            .show_ui(ui, |ui| {
                for i in 0..count {
                    ui.selectable_value(&mut state.selected, i, names.limb(i));
                }
            });
        ui.weak(format!(
            "limbs[{}] · {}",
            state.selected,
            schema::LIMBS.get(state.selected).map_or("?", |d| d.object)
        ));
    });
    let Some(limb) = data
        .get_mut("limbs")
        .and_then(|l| l.get_mut(state.selected))
        .and_then(Value::as_object_mut)
    else {
        return false;
    };
    ui.horizontal(|ui| {
        if ui
            .button(format!("✚ {}", names.text("Heal this limb")))
            .clicked()
        {
            model::heal_limb(limb, true);
            changed = true;
        }
    });
    super::card(ui, |ui| {
        ui.set_width(ui.available_width());
        for spec in schema::LIMB_FIELDS {
            changed |= field_row(ui, spec, limb, 240.0, names);
        }
    });
    if let Some(Value::Object(p)) = limb.get_mut("prostheticInfo") {
        ui.add_space(6.0);
        egui::CollapsingHeader::new(format!("🦾 {}", names.text("Prosthetic")))
            .id_salt("prosthetic")
            .default_open(p.get("hasProsthetic").and_then(Value::as_bool) == Some(true))
            .show(ui, |ui| {
                let keys: Vec<String> = p.keys().cloned().collect();
                for k in keys {
                    if let Some(v) = p.get_mut(&k) {
                        changed |= super::generic_row(ui, &k, v);
                    }
                }
                ui.weak(
                    names
                        .text("Prosthetic item ids: peglimb, steelprosthesis, musharm …")
                        .to_string(),
                );
            });
    }
    let known = |k: &str| schema::limb_field(k).is_some() || k == "prostheticInfo";
    changed |= other_fields(ui, &format!("limb{}", state.selected), limb, &known);
    changed
}

fn limb_table(ui: &mut Ui, data: &Value, names: Names, state: &mut LimbsState) {
    let Some(limbs) = data.get("limbs").and_then(Value::as_array) else {
        return;
    };
    egui::CollapsingHeader::new(names.text("All limbs overview").to_string())
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new("limb_table")
                .striped(true)
                .spacing([14.0, 4.0])
                .show(ui, |ui| {
                    for h in [
                        "Limb",
                        "Skin",
                        "Muscle",
                        "Pain",
                        "Bleeding",
                        "Infection",
                        "Status",
                    ] {
                        ui.strong(names.text(h).to_string());
                    }
                    ui.end_row();
                    for (i, limb) in limbs.iter().enumerate() {
                        if ui
                            .selectable_label(state.selected == i, names.limb(i))
                            .clicked()
                        {
                            state.selected = i;
                        }
                        let obj = limb.as_object();
                        for key in [
                            "skinHealth",
                            "muscleHealth",
                            "pain",
                            "bleedAmount",
                            "infectionAmount",
                        ] {
                            let v = obj.and_then(|o| get_f64(o, key));
                            match (schema::limb_field(key), v) {
                                (Some(spec), Some(v)) => {
                                    super::mini_gauge(ui, spec, v, 60.0);
                                }
                                _ => {
                                    ui.weak("—");
                                }
                            }
                        }
                        let p = limb_problems(limb);
                        if p.is_empty() {
                            ui.colored_label(colors::GOOD, "OK");
                        } else {
                            ui.colored_label(colors::WARN, p.join(", "));
                        }
                        ui.end_row();
                    }
                });
        });
}

/// Painted marker used in summaries: the figure without interactions.
pub fn small_figure(ui: &mut Ui, data: &Value, names: Names, height: f32) -> Option<usize> {
    let limbs = data
        .get("limbs")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    body_figure(ui, limbs, names, None, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn problems_are_listed() {
        let limb =
            json!({"skinHealth": 50.0, "muscleHealth": 80.0, "broken": true, "bleedAmount": 3.0});
        assert_eq!(limb_problems(&limb), vec!["broken", "bleeding"]);
        assert!((limb_condition(&limb) - 0.5).abs() < 1e-6);
        assert_eq!(
            limb_problems(&json!({"dismembered": true, "broken": true})),
            vec!["dismembered"]
        );
    }
}
