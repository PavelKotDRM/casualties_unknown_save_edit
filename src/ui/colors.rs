use crate::schema::{FieldSpec, Good};
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
/// Interpolates red → yellow → green for a quality in `0..=1`.
pub fn quality_color(q: f32) -> Color32 {
    let q = q.clamp(0.0, 1.0);
    let lerp = |a: Color32, b: Color32, t: f32| {
        let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
        Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
    };
    if q < 0.5 {
        lerp(BAD, WARN, q * 2.0)
    } else {
        lerp(WARN, GOOD, (q - 0.5) * 2.0)
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
    quality(spec, value).map_or(NEUTRAL, quality_color)
}
