//! egui views of the save editor.
//!
//! Each tab of the application lives in its own module. This module contains
//! widgets shared between them: colour-coded gauges similar to the in-game
//! health screen, field editors for JSON values and display-name lookup.

pub mod character;
pub mod health;
pub mod inventory;
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
            ui.label(spec.help);
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
    pub interface_language: InterfaceLanguage,
}

/// Language of the editor's own labels and menus.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceLanguage {
    /// English UI.
    #[default]
    English,
    /// Russian UI.
    Russian,
}

impl InterfaceLanguage {
    /// Configuration value.
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Russian => "ru",
        }
    }

    /// Parses a configuration value, falling back to English.
    pub fn from_code(code: Option<&str>) -> Self {
        match code {
            Some("ru") => Self::Russian,
            _ => Self::English,
        }
    }

    /// Translates a built-in English interface string.
    pub fn text<'a>(self, source: &'a str) -> Cow<'a, str> {
        let translated = match self {
            Self::English => None,
            Self::Russian => match source {
                "File" => Some("Файл"),
                "Edit" => Some("Правка"),
                "View" => Some("Вид"),
                "Game data" => Some("Данные игры"),
                "Help" => Some("Справка"),
                "Game folder:" => Some("Папка игры:"),
                "Game folder not found — showing ids" => {
                    Some("Папка игры не найдена — отображаются ID")
                }
                "Casualties Unknown installation folder" => {
                    Some("Папка установки Casualties Unknown")
                }
                "Game data loaded" => Some("Данные игры загружены"),
                "That folder does not contain CasualtiesUnknown_Data\\Lang" => {
                    Some("В этой папке нет CasualtiesUnknown_Data\\Lang")
                }
                "Game folder detected" => Some("Папка игры найдена"),
                "Game folder not found" => Some("Папка игры не найдена"),
                "Write the save (Ctrl+S)" => Some("Записать сохранение (Ctrl+S)"),
                "Redo (Ctrl+Y)" => Some("Повторить (Ctrl+Y)"),
                "Undo (Ctrl+Z)" => Some("Отменить (Ctrl+Z)"),
                "backup:" => Some("копия:"),
                "Casualties Unknown — Save Editor" => {
                    Some("Casualties Unknown — редактор сохранений")
                }
                "Save found:" => Some("Найдено сохранение:"),
                "No save at" => Some("Нет сохранения по адресу"),
                "Open another file…" => Some("Открыть другой файл…"),
                "Restoring a backup discards your unsaved changes. Save them first?" => Some(
                    "Восстановление копии отменит несохранённые изменения. Сначала сохранить их?",
                ),
                "Backups" => Some("Резервные копии"),
                "Replace the current save with this backup (the current file is backed up first)" => {
                    Some(
                        "Заменить текущее сохранение этой копией (текущий файл будет сохранён отдельно)",
                    )
                }
                "Open the backup itself for viewing/editing" => {
                    Some("Открыть копию для просмотра или редактирования")
                }
                "Casualties Unknown Save Editor" => Some("Редактор сохранений Casualties Unknown"),
                "Edits save.sv (gzip-compressed JSON). Unknown fields are preserved as-is." => {
                    Some(
                        "Редактирует save.sv (JSON в gzip). Неизвестные поля сохраняются без изменений.",
                    )
                }
                "Item, limb and setting names are read from the game's own language files." => {
                    Some(
                        "Названия предметов, конечностей и настроек берутся из языковых файлов игры.",
                    )
                }
                "Built with egui." => Some("Создано на egui."),
                "Overview" => Some("Обзор"),
                "Character" => Some("Персонаж"),
                "Unknown" => Some("Неизвестно"),
                "Unknown layer" => Some("Неизвестный этаж"),
                "Playable character (`character`)." => Some("Игровой персонаж (`character`)."),
                "Character height." => Some("Рост персонажа."),
                "Character age in years." => Some("Возраст персонажа в годах."),
                "Experiment number shown in game." => Some("Номер эксперимента в игре."),
                "Character generation version." => Some("Версия создания персонажа."),
                "Health" => Some("Здоровье"),
                "Body" => Some("Тело"),
                "Vital signs" => Some("Жизненные показатели"),
                "Quick fixes" => Some("Быстрые действия"),
                "Identity" => Some("Личные данные"),
                "Skills" => Some("Навыки"),
                "Layer:" => Some("Этаж:"),
                "m traveled" => Some("м пройдено"),
                "items" => Some("предм."),
                "cheated save" => Some("сохранение с читами"),
                "Biome" => Some("биом"),
                "Run progress" => Some("Ход забега"),
                "Run settings" => Some("Настройки забега"),
                "Mood history" => Some("История настроения"),
                "Circulation & breathing" => Some("Кровообращение и дыхание"),
                "Needs" => Some("Потребности"),
                "Mind" => Some("Психика"),
                "Sickness & immunity" => Some("Болезни и иммунитет"),
                "Movement" => Some("Движение"),
                "Blood volume" => Some("Объём крови"),
                "Blood oxygen" => Some("Насыщение крови кислородом"),
                "Heart rate" => Some("Пульс"),
                "Blood pressure" => Some("Артериальное давление"),
                "Respiratory rate" => Some("Эффективность дыхания"),
                "Fibrillation" => Some("Фибрилляция"),
                "Forced fibrillation" => Some("Принудительная фибрилляция"),
                "Internal bleeding" => Some("Внутреннее кровотечение"),
                "Hemothorax" => Some("Гемоторакс"),
                "Pulmonary embolism" => Some("Тромбоэмболия лёгочной артерии"),
                "Stroke" => Some("Инсульт"),
                "Blood viscosity" => Some("Вязкость крови"),
                "Blood vessel size" => Some("Диаметр сосудов"),
                "Heart-rate pressure offset" => Some("Влияние пульса на давление"),
                "Pressure change (medicine)" => Some("Изменение давления (лекарства)"),
                "Adrenaline" => Some("Адреналин"),
                "Current adrenaline" => Some("Текущий адреналин"),
                "Satiety" => Some("Насыщение"),
                "Hydration" => Some("Уровень воды"),
                "Stamina" => Some("Выносливость"),
                "Energy" => Some("Энергия"),
                "Caffeine" => Some("Кофеин"),
                "Weight offset" => Some("Изменение веса"),
                "Dirtiness" => Some("Загрязнение"),
                "Wetness" => Some("Влажность"),
                "Snow" => Some("Снег"),
                "Bad sleep" => Some("Плохой сон"),
                "Good sleep time" => Some("Время хорошего сна"),
                "Mood" => Some("Настроение"),
                "Level" => Some("Уровень"),
                "Experience" => Some("Опыт"),
                "Gain multiplier" => Some("Множитель опыта"),
                "Age" => Some("Возраст"),
                "Height (cm)" => Some("Рост (см)"),
                "cm" => Some("см"),
                "Blood, oxygen, heart, consciousness, shock, sickness, temperature…" => {
                    Some("Кровь, кислород, сердце, сознание, шок, болезни, температура…")
                }
                "Food, water, stamina, energy; clean and dry" => {
                    Some("Еда, вода, выносливость и энергия; чистота и сухость")
                }
                "Experiment id" => Some("Номер эксперимента"),
                "Version" => Some("Версия"),
                "Limbs" => Some("Конечности"),
                "Inventory" => Some("Инвентарь"),
                "Run" => Some("Забег"),
                "Raw JSON" => Some("Исходный JSON"),
                "Condition (healthy → injured)" => Some("Состояние (здоровое → травмированное)"),
                "Bleeding" => Some("Кровотечение"),
                "Infection" => Some("Инфекция"),
                "Fracture" => Some("Перелом"),
                "Dislocation" => Some("Вывих"),
                "Tourniquet" => Some("Жгут"),
                "Dismembered" => Some("Ампутация"),
                "Prosthetic" => Some("Протез"),
                "Prosthetic item ids: peglimb, steelprosthesis, musharm …" => {
                    Some("ID протезов: peglimb, steelprosthesis, musharm …")
                }
                "This save has no `limbs` array." => {
                    Some("В сохранении отсутствует массив `limbs`.")
                }
                "Heal this limb" => Some("Вылечить эту конечность"),
                "All limbs overview" => Some("Обзор конечностей"),
                "Limb" => Some("Конечность"),
                "Head" => Some("Голова"),
                "Thorax" => Some("Грудная клетка"),
                "Abdomen" => Some("Живот"),
                "Upper right arm" => Some("Правая плечевая часть"),
                "Right forearm" => Some("Правое предплечье"),
                "Right hand" => Some("Правая кисть"),
                "Upper left arm" => Some("Левая плечевая часть"),
                "Left forearm" => Some("Левое предплечье"),
                "Left hand" => Some("Левая кисть"),
                "Right thigh" => Some("Правое бедро"),
                "Right crus" => Some("Правая голень"),
                "Right foot" => Some("Правая стопа"),
                "Left thigh" => Some("Левое бедро"),
                "Left crus" => Some("Левая голень"),
                "Left foot" => Some("Левая стопа"),
                "Strength" => Some("Сила"),
                "Resilience" => Some("Стойкость"),
                "Intelligence" => Some("Интеллект"),
                "Skin" => Some("Кожа"),
                "Muscle" => Some("Мышцы"),
                "Pain" => Some("Боль"),
                "Status" => Some("Статус"),
                "OK" => Some("В норме"),
                "Open game save" => Some("Открыть сохранение игры"),
                "Open…" => Some("Открыть…"),
                "Reload from disk" => Some("Перезагрузить с диска"),
                "Save" => Some("Сохранить"),
                "Save as…" => Some("Сохранить как…"),
                "Export readable JSON…" => Some("Экспорт читаемого JSON…"),
                "Import JSON…" => Some("Импорт JSON…"),
                "Create backup now" => Some("Создать резервную копию"),
                "Backups…" => Some("Резервные копии…"),
                "Open save folder" => Some("Открыть папку сохранения"),
                "Exit" => Some("Выход"),
                "Undo" => Some("Отменить"),
                "Redo" => Some("Повторить"),
                "Revert all changes" => Some("Отменить все изменения"),
                "Heal all limbs" => Some("Вылечить все конечности"),
                "Heal all + regrow dismembered" => {
                    Some("Вылечить и восстановить отсечённые конечности")
                }
                "Heal + regrow limbs" => Some("Вылечить и восстановить конечности"),
                "Restore vitals" => Some("Восстановить показатели"),
                "Fill needs" => Some("Восполнить потребности"),
                "Language of names:" => Some("Язык названий игры:"),
                "Interface language:" => Some("Язык интерфейса:"),
                "English" => Some("Английский"),
                "Russian" => Some("Русский"),
                "Yes" => Some("Да"),
                "No" => Some("Нет"),
                "Layer" => Some("Этаж"),
                "Player" => Some("Игрок"),
                "Trader" => Some("Торговец"),
                "Challenges" => Some("Испытания"),
                "Unchipped" => Some("Без чипа"),
                "Endless" => Some("Бесконечный режим"),
                "Base loot density" => Some("Базовая плотность добычи"),
                "Loot multiplier per layer" => Some("Множитель добычи на этаж"),
                "Base trap density" => Some("Базовая плотность ловушек"),
                "Trap increase per layer" => Some("Рост числа ловушек на этаж"),
                "Ambient light" => Some("Освещённость"),
                "Time limit" => Some("Ограничение времени"),
                "Ore amount" => Some("Количество руды"),
                "Layer modifier chance" => Some("Шанс модификатора этажа"),
                "Time between earthquakes" => Some("Интервал между землетрясениями"),
                "Temperature offset" => Some("Смещение температуры"),
                "Allow layer-end movement" => Some("Разрешить перемещение в конце этажа"),
                "Starting supplies" => Some("Начальные припасы"),
                "XP gain" => Some("Получение опыта"),
                "Metabolism rate" => Some("Скорость метаболизма"),
                "Healing rate" => Some("Скорость лечения"),
                "Fracture pain" => Some("Боль от перелома"),
                "Bleed rate" => Some("Скорость кровотечения"),
                "Infection speed" => Some("Скорость инфекции"),
                "Infection chance" => Some("Шанс инфекции"),
                "Fibrillation rate" => Some("Скорость фибрилляции"),
                "Mood normalisation rate" => Some("Скорость нормализации настроения"),
                "Bonus limb armor" => Some("Дополнительная защита конечностей"),
                "Stamina regeneration" => Some("Восстановление выносливости"),
                "Attack damage" => Some("Урон от атак"),
                "Minigame hand shake" => Some("Дрожание рук в мини-играх"),
                "Sleep cycle speed" => Some("Скорость цикла сна"),
                "Encumbrance cap" => Some("Предел нагрузки"),
                "Insulation multiplier" => Some("Множитель теплоизоляции"),
                "Strokes" => Some("Инсульты"),
                "Brain damage effects" => Some("Последствия повреждения мозга"),
                "Force sleep" => Some("Принудительный сон"),
                "Low mood events" => Some("События при плохом настроении"),
                "Liquid pushing" => Some("Перемещение жидкости"),
                "Disfigurement" => Some("Обезображивание"),
                "Crush depth" => Some("Глубина раздавливания"),
                "No sleep restrictions" => Some("Без ограничений сна"),
                "Infinite last stand" => Some("Бесконечное последнее сопротивление"),
                "Trader chance" => Some("Шанс появления торговца"),
                "Trader item amount" => Some("Количество предметов у торговца"),
                "Trader reputation offset" => Some("Смещение репутации у торговца"),
                "Enemy strength" => Some("Сила врагов"),
                "Item decay rate" => Some("Скорость порчи предметов"),
                "Lockpick precision" => Some("Точность взлома замков"),
                "Permanent addiction" => Some("Постоянная зависимость"),
                "Mind-wiped" => Some("Потеря памяти"),
                "Zero skills" => Some("Навыки обнулены"),
                "Blind" => Some("Слепота"),
                "Amputee" => Some("Ампутация конечности"),
                "Immortal snail" => Some("Бессмертная улитка"),
                "None" => Some("Нет"),
                "Enough" => Some("Достаточно"),
                "Bright" => Some("Ярко"),
                "Light" => Some("Немного"),
                "Full" => Some("Полный набор"),
                "Run time (s)" => Some("Время забега (с)"),
                "Layer index the run resumes on (`biome`)." => {
                    Some("Номер этажа, с которого продолжается забег (`biome`).")
                }
                "Total time of the run in seconds." => Some("Общее время забега в секундах."),
                "Distance traveled" => Some("Пройденное расстояние"),
                "Total distance traveled in the run." => Some("Общее пройденное расстояние."),
                "Endless loops" => Some("Циклы бесконечного режима"),
                "Number of completed endless-mode loops." => {
                    Some("Количество завершённых циклов бесконечного режима.")
                }
                "Calories consumed" => Some("Потреблено калорий"),
                "Calories eaten during the run." => Some("Количество калорий, съеденных за забег."),
                "Trap rarity" => Some("Редкость ловушек"),
                "Trap rarity multiplier." => Some("Множитель редкости ловушек."),
                "Loot rarity" => Some("Редкость добычи"),
                "Loot rarity multiplier." => Some("Множитель редкости добычи."),
                "Mercenary run" => Some("Забег наёмника"),
                "The run was started as a mercenary." => Some("Забег начат наёмником."),
                "Cheated save" => Some("Сохранение с читами"),
                "Marks the save as cheated; achievements are disabled." => {
                    Some("Помечает сохранение как читерское; достижения отключаются.")
                }
                "Achievements are disabled for cheated saves." => {
                    Some("Для сохранений с читами достижения отключены.")
                }
                "⚠ Achievements are disabled for cheated saves." => {
                    Some("⚠ Для сохранений с читами достижения отключены.")
                }
                "No run settings in this save." => Some("В этом сохранении нет настроек забега."),
                "No body data." => Some("Нет данных о теле."),
                "No `body.skills` in this save." => Some("В сохранении отсутствуют `body.skills`."),
                "This save has no `body` object." => {
                    Some("В сохранении отсутствует объект `body`.")
                }
                "Click a limb to edit it." => Some("Нажмите на конечность, чтобы изменить её."),
                "Changing a level recalculates the experience bounds exactly like the game does, keeping the progress towards the next level." => {
                    Some(
                        "При изменении уровня границы опыта пересчитываются как в игре, сохраняя прогресс до следующего уровня.",
                    )
                }
                "Experience within the current level" => Some("Опыт на текущем уровне"),
                "Heal limbs" => Some("Вылечить конечности"),
                "Full recovery" => Some("Полное восстановление"),
                "All of the above" => Some("Всё перечисленное выше"),
                "Heal all limbs (keeps dismembered limbs missing)" => {
                    Some("Вылечить конечности (отсечённые не восстанавливаются)")
                }
                "Every change can be undone with Ctrl+Z; nothing is written until you save." => {
                    Some(
                        "Изменения можно отменить сочетанием Ctrl+Z. Файл не изменится до сохранения.",
                    )
                }
                "Unsaved changes" => Some("Несохранённые изменения"),
                "Save your changes before closing?" => Some("Сохранить изменения перед выходом?"),
                "Save your changes before continuing?" => {
                    Some("Сохранить изменения перед продолжением?")
                }
                "Discard" => Some("Не сохранять"),
                "Cancel" => Some("Отмена"),
                "About" => Some("О программе"),
                "No backups yet." => Some("Резервных копий пока нет."),
                "Open folder" => Some("Открыть папку"),
                "Backup now" => Some("Создать копию"),
                "Restore" => Some("Восстановить"),
                "Open" => Some("Открыть"),
                "Current file" => Some("Текущий файл"),
                "modified" => Some("изменён"),
                "saved" => Some("сохранён"),
                "The game writes save.sv on \"Save & Quit\" and deletes it when the run is loaded again." => {
                    Some(
                        "Игра создаёт save.sv при выборе «Сохранить и выйти» и удаляет файл при загрузке забега.",
                    )
                }
                "Cannot determine the save folder." => {
                    Some("Не удалось определить папку сохранений.")
                }
                "A timestamped backup is created in SaveEditorBackups every time a save is opened." => {
                    Some(
                        "При каждом открытии сохранения создаётся резервная копия в SaveEditorBackups.",
                    )
                }
                "The first item of a location is held / worn; the following ones are stored inside it." => {
                    Some(
                        "Первый предмет в ячейке удерживается или надет; остальные хранятся внутри него.",
                    )
                }
                "No saved component state (the game uses the item's defaults)." => Some(
                    "Сохранённое состояние компонента отсутствует (используются значения игры).",
                ),
                "Add liquid contents" => Some("Добавить содержимое"),
                "Add liquid" => Some("Добавить жидкость"),
                "Empty" => Some("Опустошить"),
                "Set game folder…" => Some("Выбрать папку игры…"),
                "Detect automatically" => Some("Найти автоматически"),
                "Add item…" => Some("Добавить предмет…"),
                "Add item" => Some("Добавить предмет"),
                "Location" => Some("Расположение"),
                "Inventory slot" => Some("Ячейка инвентаря"),
                "Worn" => Some("Надето"),
                "Unassigned (slot -1, no wear slot)" => Some("Не назначено (нет слота одежды)"),
                "Select an item to edit it." => Some("Выберите предмет для редактирования."),
                "The inventory is empty." => Some("Инвентарь пуст."),
                "Item id" => Some("ID предмета"),
                "Favourited" => Some("Избранное"),
                "Label" => Some("Метка"),
                "Swatch" => Some("Вариант цвета"),
                "Components" => Some("Компоненты"),
                "Weight" => Some("Вес"),
                "Weight:" => Some("Вес:"),
                "Condition" => Some("Состояние"),
                "Delete" => Some("Удалить"),
                "Duplicate" => Some("Дублировать"),
                "Move down" => Some("Переместить вниз"),
                "Move up" => Some("Переместить вверх"),
                "Favourite" => Some("В избранное"),
                "Unknown id — not in the game's item list." => {
                    Some("Неизвестный ID — отсутствует в списке предметов игры.")
                }
                "⚠ This item belongs in wear slot {expected}; it is assigned to {actual}." => {
                    Some(
                        "⚠ Предмет предназначен для слота одежды «{expected}», но назначен в «{actual}».",
                    )
                }
                "contains" => Some("содержит"),
                "item(s):" => Some("предм.:"),
                "wearable" => Some("одежда"),
                "armor" => Some("защита"),
                "insulation" => Some("теплоизоляция"),
                "unknown id" => Some("неизвестный ID"),
                "custom name" => Some("своё название"),
                "Color variant index" => Some("Индекс варианта цвета"),
                "Load" => Some("Нагрузка"),
                "Carry capacity cannot be calculated from this save." => {
                    Some("Не удалось рассчитать грузоподъёмность по этому сохранению.")
                }
                "Incomplete: some item or container stats are unavailable." => {
                    Some("Расчёт неполный: нет данных о некоторых предметах или контейнерах.")
                }
                "⚠ Character is overburdened." => Some("⚠ Персонаж перегружен."),
                "Item stats unavailable; sync game data to show verified weight." => Some(
                    "Статистика предмета недоступна. Синхронизируйте данные игры, чтобы увидеть вес.",
                ),
                "Max charge" => Some("Максимальный заряд"),
                "Battery" => Some("Батарея"),
                "Remove this component (use defaults)" => {
                    Some("Удалить компонент (использовать значения по умолчанию)")
                }
                "Fill to capacity" => Some("Заполнить до максимума"),
                "Scales all liquids proportionally" => {
                    Some("Пропорционально изменяет количество всех жидкостей")
                }
                "Category:" => Some("Категория:"),
                "Search name or id" => Some("Поиск по названию или ID"),
                "All categories" => Some("Все категории"),
                "Nothing found." => Some("Ничего не найдено."),
                "Put into" => Some("Поместить в"),
                "The location is occupied: the item will be stored inside the held / worn item." => {
                    Some(
                        "Ячейка занята: предмет будет помещён внутрь удерживаемого или надетого предмета.",
                    )
                }
                "The location is empty: the item will be held / worn." => {
                    Some("Ячейка свободна: предмет будет удерживаться или надет.")
                }
                "Add" => Some("Добавить"),
                "Close" => Some("Закрыть"),
                "Tree" => Some("Дерево"),
                "Text" => Some("Текст"),
                "Filter keys / values" => Some("Фильтр ключей и значений"),
                "Expand all" => Some("Развернуть всё"),
                "Collapse all" => Some("Свернуть всё"),
                "Apply" => Some("Применить"),
                "Reload from data" => Some("Обновить из данных"),
                "Unapplied text changes" => Some("Текстовые изменения не применены"),
                "Other" => Some("Прочее"),
                "Recent mood samples (oldest → newest). Drag points to edit." => Some(
                    "Недавние значения настроения (от старых к новым). Перетаскивайте точки для изменения.",
                ),
                "Saved recipe data" => Some("Сохранённые рецепты"),
                "marked" => Some("отмечено"),
                "`savedRecipeData.saved`: per-recipe flag (Item1) and value (Item2), in game order." => {
                    Some(
                        "`savedRecipeData.saved`: отметка рецепта (Item1) и значение (Item2) в порядке игры.",
                    )
                }
                "Mark all" => Some("Отметить все"),
                "Unmark all" => Some("Снять все отметки"),
                "Click or drag to set the value" => {
                    Some("Нажмите или перетащите, чтобы изменить значение")
                }
                _ => None,
            },
        };
        translated.map_or(Cow::Borrowed(source), Cow::Borrowed)
    }
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
        let from_locale = self
            .locale
            .and_then(|l| l.other(&format!("layertitle{}", biome + 1)))
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
}
