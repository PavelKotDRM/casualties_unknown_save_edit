pub(super) fn translate(source: &str) -> Option<&'static str> {
    match source {
        "Game load" => Some("Нагрузка в игре"),
        "Game item weight" => Some("Вес предмета в игре"),
        "Game item weight:" => Some("Вес предмета в игре:"),
        "Game load is shown in game units (u). Container capacity and per-item limits determine what can be stored; the encumbrance multiplier reduces the contribution of stored items. Liquid containers scale with their fill level." => {
            Some(
                "Игровая нагрузка отображается в игровых единицах (u). Вместимость и лимит веса предмета определяют, что можно хранить; коэффициент контейнера уменьшает вклад содержимого. Вес ёмкостей зависит от их наполненности.",
            )
        }
        "Capacity remaining:" => Some("Остаток вместимости:"),
        "Max weight/item:" => Some("Макс. вес предмета:"),
        "Encumbrance reduction:" => Some("Снижение нагрузки:"),
        "Remaining capacity is maximum weight minus the full weight of stored items; encumbrance reduction affects carried load, not storage capacity." => {
            Some(
                "Остаток вместимости равен максимальному весу за вычетом полного веса хранящихся предметов; снижение нагрузки влияет на переносимый вес, но не на вместимость.",
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
        "Approximation: liquids use 1 kg per litre; game weight units are treated as kg for empty item weight. Container encumbrance modifiers do not apply." => {
            Some(
                "Приблизительно: для жидкостей используется 1 кг на литр; игровые единицы веса для пустых предметов условно считаются килограммами. Коэффициенты контейнеров не применяются.",
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
        "Game folder not found — showing ids" => Some("Папка игры не найдена — отображаются ID"),
        "Casualties Unknown installation folder" => Some("Папка установки Casualties Unknown"),
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
        "Casualties Unknown — Save Editor" => Some("Casualties Unknown — редактор сохранений"),
        "Save found:" => Some("Найдено сохранение:"),
        "No save at" => Some("Нет сохранения по адресу"),
        "Open another file…" => Some("Открыть другой файл…"),
        "Restoring a backup discards your unsaved changes. Save them first?" => {
            Some("Восстановление копии отменит несохранённые изменения. Сначала сохранить их?")
        }
        "Backups" => Some("Резервные копии"),
        "Replace the current save with this backup (the current file is backed up first)" => {
            Some("Заменить текущее сохранение этой копией (текущий файл будет сохранён отдельно)")
        }
        "Open the backup itself for viewing/editing" => {
            Some("Открыть копию для просмотра или редактирования")
        }
        "Casualties Unknown Save Editor" => Some("Редактор сохранений Casualties Unknown"),
        "Edits save.sv (gzip-compressed JSON). Unknown fields are preserved as-is." => {
            Some("Редактирует save.sv (JSON в gzip). Неизвестные поля сохраняются без изменений.")
        }
        "Item, limb and setting names are read from the game's own language files." => {
            Some("Названия предметов, конечностей и настроек берутся из языковых файлов игры.")
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

        _ => None,
    }
}
