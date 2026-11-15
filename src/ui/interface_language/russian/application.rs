pub(super) fn translate(source: &str) -> Option<&'static str> {
    match source {
        "New save…" => Some("Новое сохранение…"),
        "Create new save…" => Some("Создать новое сохранение…"),
        "Create new save" => Some("Создать новое сохранение"),
        "Create save…" => Some("Создать сохранение…"),
        "World settings" => Some("Настройки мира"),
        "World preset:" => Some("Шаблон мира:"),
        "Starting layer:" => Some("Начальный этаж:"),
        "Choose a built-in game preset or customize every world setting." => {
            Some("Выберите игровой шаблон или настройте каждый параметр мира.")
        }
        "Choose the character after setting up the world." => {
            Some("Выберите персонажа после настройки мира.")
        }
        "Next" => Some("Далее"),
        "Back" => Some("Назад"),
        "Custom" => Some("Настраиваемый"),
        "Normal" => Some("Обычный"),
        "Relaxed" => Some("Облегчённый"),
        "Paradise" => Some("Рай"),
        "Desolate" => Some("Безлюдный"),
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
        "Heal all + regrow dismembered" => Some("Вылечить и восстановить отсечённые конечности"),
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
        "This save has no `body` object." => Some("В сохранении отсутствует объект `body`."),
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
            Some("Изменения можно отменить сочетанием Ctrl+Z. Файл не изменится до сохранения.")
        }
        "Unsaved changes" => Some("Несохранённые изменения"),
        "Save your changes before closing?" => Some("Сохранить изменения перед выходом?"),
        "Save your changes before continuing?" => Some("Сохранить изменения перед продолжением?"),
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
        "Cannot determine the save folder." => Some("Не удалось определить папку сохранений."),
        "A timestamped backup is created in SaveEditorBackups every time a save is opened." => {
            Some("При каждом открытии сохранения создаётся резервная копия в SaveEditorBackups.")
        }

        _ => None,
    }
}
