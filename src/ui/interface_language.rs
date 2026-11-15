//! Built-in interface translations.

use std::borrow::Cow;

use crate::interface_packs::InterfacePack;

/// Language of the editor's own labels and menus.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceLanguage<'a> {
    /// English UI.
    #[default]
    English,
    /// Russian UI.
    Russian,
    /// A user-provided interface language.
    External(&'a InterfacePack),
}

impl<'p> InterfaceLanguage<'p> {
    /// Configuration value.
    pub fn code(self) -> &'p str {
        match self {
            Self::English => "en",
            Self::Russian => "ru",
            Self::External(pack) => &pack.code,
        }
    }

    /// Parses a configuration value, falling back to English.
    pub fn from_code(code: Option<&str>, pack: Option<&'p InterfacePack>) -> Self {
        match code {
            Some("ru") => Self::Russian,
            Some("en") | None => Self::English,
            Some(code) => pack
                .filter(|p| p.code == code)
                .map_or(Self::English, Self::External),
        }
    }

    /// Translates a built-in English interface string.
    pub fn text<'a>(self, source: &'a str) -> Cow<'a, str> {
        let translated = match self {
            Self::English => None,
            Self::External(pack) => {
                return pack
                    .translations
                    .get(source)
                    .filter(|text| !text.trim().is_empty())
                    .map_or(Cow::Borrowed(source), |text| Cow::Owned(text.clone()));
            }
            Self::Russian => match source {
                "Game load" => Some("Нагрузка в игре"),
                "Game item weight" => Some("Вес предмета в игре"),
                "Game item weight:" => Some("Вес предмета в игре:"),
                "Game load uses item weights and container encumbrance multipliers. Liquid containers scale with their fill level." => {
                    Some(
                        "Игровая нагрузка учитывает вес предметов и коэффициенты контейнеров. Вес ёмкостей зависит от их наполненности.",
                    )
                }
                "Incomplete: item, container or liquid data is unavailable." => {
                    Some("Расчёт неполный: нет данных о предмете, контейнере или жидкости.")
                }
                "Physical weight" => Some("Физический вес"),
                "Physical weight:" => Some("Физический вес:"),
                "Empty container:" => Some("Пустая ёмкость:"),
                "Liquid:" => Some("Жидкость:"),
                "Physical estimate incomplete" => Some("Физический вес рассчитан не полностью"),
                "Item weight unavailable." => Some("Вес предмета недоступен."),
                "Liquid contents are missing or invalid; weight estimates are incomplete." => {
                    Some("Состав жидкости отсутствует или некорректен; расчёт веса неполный.")
                }
                "Approximation: liquids use 1 kg per litre; empty container weight is inferred from the game's minimum item weight. Container encumbrance modifiers do not apply." => {
                    Some(
                        "Приблизительно: для жидкостей используется 1 кг на литр; вес пустой ёмкости оценивается по минимальному игровому весу предмета. Коэффициенты контейнеров не применяются.",
                    )
                }
                "Level {level}: {min} XP, level {next}: {max} XP" => {
                    Some("Уровень {level}: {min} опыта, уровень {next}: {max} опыта")
                }
                "Reload interface languages" => Some("Обновить языки интерфейса"),
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
