//! egui views of the save editor.
//!
//! Each tab of the application lives in its own module. This module contains
//! widgets shared between them: colour-coded gauges similar to the in-game
//! health screen, field editors for JSON values and display-name lookup.

pub mod about;
pub mod achievements;
pub mod backups;
pub mod character;
pub mod health;
mod interface_language;
pub mod inventory;
pub use interface_language::InterfaceLanguage;
pub mod limbs;
pub mod overview;
pub mod raw;
pub mod report;
pub mod run;
pub mod spoilers;
pub mod wiki;

pub mod colors;
mod names;
mod text;
mod widgets;

pub use colors::{quality, quality_color, value_color};
pub use names::Names;
pub use text::{category_icon, fmt_num, prettify_id, strip_rich_text};
pub use widgets::{
    bounded_window, card, field_row, gauge_bar, generic_row, mini_gauge, number_editor,
    other_fields, section_heading,
};

/// Width of the label column in field rows.
pub const LABEL_WIDTH: f32 = 190.0;

/// Reports an edit failure to the application and indicates whether it succeeded.
pub fn operation_result(ui: &egui::Ui, result: Result<(), String>) -> bool {
    match result {
        Ok(()) => true,
        Err(error) => {
            ui.ctx().data_mut(|data| {
                data.insert_temp(egui::Id::unique("model_operation_error"), error);
            });
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::Locale;
    use crate::schema;
    use egui::Vec2;

    #[test]
    #[allow(deprecated)]
    fn model_operation_errors_are_reported_to_the_application() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            assert!(!operation_result(ui, Err("missing data".to_owned())));
            assert!(operation_result(ui, Ok(())));
        });
        output.textures_delta.clear();
        assert_eq!(
            egui::Id::new("model_operation_error"),
            egui::Id::unique("model_operation_error")
        );
        assert_eq!(
            ctx.data_mut(
                |data| data.remove_temp::<String>(egui::Id::unique("model_operation_error"))
            ),
            Some("missing data".to_owned())
        );
    }

    #[test]
    fn quality_follows_good_direction() {
        let pain = schema::FieldSpec {
            key: "pain",
            label: "Pain",
            help: "",
            kind: schema::Kind::Gauge,
            min: 0.0,
            max: 100.0,
            unit: "",
            good: schema::Good::Low,
        };
        assert_eq!(quality(&pain, 0.0), Some(1.0));
        assert_eq!(quality(&pain, 100.0), Some(0.0));
        let hr = schema::FieldSpec {
            good: schema::Good::Near(70.0),
            max: 200.0,
            ..pain
        };
        assert!(quality(&hr, 70.0).unwrap() > quality(&hr, 180.0).unwrap());
    }

    #[test]
    fn bounded_window_fits_a_small_screen_and_scrolls_with_the_wheel() {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 500.0));
        let frame = |events: Vec<egui::Event>| {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            };
            let mut measured = (egui::Rect::NOTHING, egui::Rect::NOTHING, Vec2::ZERO);
            let mut output = ctx.run_ui(input, |ui| {
                let ctx = ui.ctx().clone();
                let shown =
                    bounded_window(&ctx, "test", Vec2::new(760.0, 680.0), 540.0).show(&ctx, |ui| {
                        let out = egui::ScrollArea::vertical()
                            .auto_shrink(false)
                            .max_height(ui.available_height() - 40.0)
                            .show(ui, |ui| {
                                for i in 0..200 {
                                    ui.label(format!("row {i}"));
                                }
                            });
                        measured.1 = out.inner_rect;
                        measured.2 = out.state.clamped_offset();
                        ui.separator();
                        ui.button("footer")
                    });
                measured.0 = shown.unwrap().response.rect;
            });
            output.textures_delta.clear();
            measured
        };
        for _ in 0..2 {
            frame(Vec::new());
        }
        let (window_rect, scroll_rect, offset) = frame(Vec::new());
        assert!(
            screen.contains_rect(window_rect),
            "{window_rect:?} leaves {screen:?}"
        );
        assert_eq!(offset.y, 0.0);
        frame(vec![
            egui::Event::PointerMoved(scroll_rect.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -300.0),
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Unknown,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        let mut measured = frame(Vec::new());
        for _ in 0..30 {
            measured = frame(Vec::new());
        }
        let (window_rect, _, offset) = measured;
        assert!(offset.y > 0.0, "the wheel did not scroll the content");
        assert!(screen.contains_rect(window_rect));
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
        assert_eq!(
            InterfaceLanguage::Russian.text("Capacity remaining:"),
            "Остаток вместимости:"
        );
        assert_eq!(
            InterfaceLanguage::Russian.text("Over capacity by {weight} u"),
            "Превышена вместимость на {weight} u"
        );
        assert_eq!(
            InterfaceLanguage::Russian.text("Space left: {weight} u"),
            "Свободно: {weight} u"
        );
        assert_eq!(
            InterfaceLanguage::Russian.text("Inside {item}: full"),
            "Внутри «{item}»: места нет"
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
