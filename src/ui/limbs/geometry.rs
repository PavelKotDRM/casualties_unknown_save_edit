//! Body-figure layout in a 200 × 400 design space.

use egui::{Rect, Vec2, vec2};

/// Viewer-left is the character's right side (`F` limbs), as on the game screen.
pub(super) const FIGURE: [(f32, f32, f32, f32); 15] = [
    (75.0, 5.0, 125.0, 62.0),
    (68.0, 68.0, 132.0, 148.0),
    (72.0, 150.0, 128.0, 214.0),
    (40.0, 70.0, 64.0, 144.0),
    (38.0, 146.0, 62.0, 212.0),
    (34.0, 214.0, 62.0, 244.0),
    (136.0, 70.0, 160.0, 144.0),
    (138.0, 146.0, 162.0, 212.0),
    (138.0, 214.0, 166.0, 244.0),
    (72.0, 218.0, 98.0, 292.0),
    (74.0, 294.0, 97.0, 362.0),
    (62.0, 364.0, 97.0, 384.0),
    (102.0, 218.0, 128.0, 292.0),
    (103.0, 294.0, 126.0, 362.0),
    (103.0, 364.0, 138.0, 384.0),
];

pub(super) fn limb_rect(bounds: Rect, (x0, y0, x1, y1): (f32, f32, f32, f32), scale: f32) -> Rect {
    Rect::from_min_max(
        bounds.min + vec2(x0, y0) * scale,
        bounds.min + Vec2::new(x1, y1) * scale,
    )
}
