pub(super) fn translate(source: &str) -> Option<&'static str> {
    match source {
        "Game wiki" => Some("Вики игры"),
        "Search all sections…" => Some("Поиск по всем разделам…"),
        "Game language files not found: descriptions are limited." => {
            Some("Языковые файлы игры не найдены: описания ограничены.")
        }
        "Nothing found." => Some("Ничего не найдено."),
        "👤 Characters" => Some("👤 Персонажи"),
        "📈 Skills" => Some("📈 Навыки"),
        "❤ Body stats" => Some("❤ Показатели тела"),
        "🦴 Limbs" => Some("🦴 Конечности"),
        "🌡 Conditions" => Some("🌡 Состояния"),
        "⚙ World settings" => Some("⚙ Настройки мира"),
        "🎚 Presets" => Some("🎚 Пресеты"),
        "🗺 Layers" => Some("🗺 Слои"),
        "🎒 Items" => Some("🎒 Предметы"),
        "Playable characters. Each one is unlocked by finishing a run with the previous character." => {
            Some("Игровые персонажи. Каждый открывается после прохождения забега за предыдущего.")
        }
        "Skills grow with experience. Each character has its own starting levels and experience multipliers." => {
            Some(
                "Навыки растут с опытом. У каждого персонажа свои стартовые уровни и множители опыта.",
            )
        }
        "Whole-body stats stored in the save's body object." => {
            Some("Показатели всего тела из объекта body сохранения.")
        }
        "Every limb stores the same set of stats." => {
            Some("Каждая конечность хранит одинаковый набор показателей.")
        }
        "Status effects (moodles) shown by the game, with its own descriptions." => {
            Some("Эффекты состояния (мудлы) с описаниями из самой игры.")
        }
        "World settings chosen when a run starts. Defaults are the values of the standard preset." => {
            Some(
                "Настройки мира, выбираемые в начале забега. По умолчанию — значения стандартного пресета.",
            )
        }
        "Built-in world presets and the settings they change." => {
            Some("Встроенные пресеты мира и изменяемые ими настройки.")
        }
        "World layers and the random modifiers a layer can get." => {
            Some("Слои мира и случайные модификаторы, которые может получить слой.")
        }
        "Items known to the game with their stats from the installation. Effects are read from the game's code: amounts are per use unless marked otherwise." => {
            Some(
                "Известные игре предметы с характеристиками из установленной игры. Эффекты прочитаны из кода игры: значения указаны за одно использование, если не отмечено иное.",
            )
        }
        "🧪 Liquids" => Some("🧪 Жидкости"),
        "Liquids that containers can hold, with the effects of drinking or injecting 100 ml, read from the game's code." => {
            Some(
                "Жидкости, которые могут храниться в ёмкостях, с эффектами от 100 мл при питье или инъекции, прочитанными из кода игры.",
            )
        }
        "Ctrl+F to search, Esc to clear. Every word must match." => {
            Some("Ctrl+F — поиск, Esc — очистить. Должны совпасть все слова.")
        }
        "{count} results" => Some("Найдено: {count}"),
        "Value" => Some("Цена"),
        "Spoils after" => Some("Портится через"),
        "Recognised with" => Some("Распознаётся при"),
        "INT {level}" => Some("ИНТ {level}"),
        "Crafting qualities" => Some("Свойства для крафта"),
        "Contents" => Some("Содержимое"),
        "Jump height" => Some("Высота прыжка"),
        "Properties" => Some("Особенности"),
        "can be combined" => Some("можно объединять"),
        "usable while depressed" => Some("можно использовать в депрессии"),
        "cannot be traded" => Some("нельзя продать"),
        "Used on a limb" => Some("При использовании на конечности"),
        "When used" => Some("При использовании"),
        "≈{count} uses per item" => Some("≈{count} использований на предмет"),
        "Drinks {ml} ml of its contents" => Some("Выпивается {ml} мл содержимого"),
        "Injects {ml} ml of its contents" => Some("Вводится {ml} мл содержимого"),
        "Applies {ml} ml of its contents" => Some("Наносится {ml} мл содержимого"),
        "Possible effects of the contents" => Some("Возможные эффекты содержимого"),
        "Possible effects" => Some("Возможные эффекты"),
        "Value per litre" => Some("Цена за литр"),
        "Injectable" => Some("Можно вводить"),
        "Comes in" => Some("Встречается в"),
        "Drinking 100 ml" => Some("Если выпить 100 мл"),
        "Injecting or applying 100 ml" => Some("Если ввести или нанести 100 мл"),
        "{n} s" => Some("{n} с"),
        "{n} min" => Some("{n} мин"),
        "{n} h" => Some("{n} ч"),
        "Opioids" => Some("Опиоиды"),
        "Opioid antagonist" => Some("Антагонист опиоидов"),
        "Opioid tolerance" => Some("Толерантность к опиоидам"),
        "Sleeping pills" => Some("Снотворное"),
        "Disinfection" => Some("Дезинфекция"),
        "Vomiting" => Some("Рвота"),
        "All limbs" => Some("Все конечности"),
        "on" => Some("вкл"),
        "off" => Some("выкл"),
        "≈{kcal} kcal" => Some("≈{kcal} ккал"),
        "over {time}" => Some("за {time}"),
        "for {time}" => Some("на {time}"),
        "whole item" => Some("весь предмет"),
        "Unlock state unknown" => Some("Состояние открытия неизвестно"),
        "Unlocked" => Some("Открыт"),
        "Locked" => Some("Закрыт"),
        "level" => Some("уровень"),
        "XP" => Some("опыт"),
        "Unlock" => Some("Открытие"),
        "Save value" => Some("Значение в сохранении"),
        "Save keys" => Some("Ключи в сохранении"),
        "Experience per level" => Some("Опыт по уровням"),
        "Total experience needed to reach each level." => {
            Some("Суммарный опыт, необходимый для каждого уровня.")
        }
        "Type" => Some("Тип"),
        "on/off" => Some("вкл/выкл"),
        "whole number" => Some("целое число"),
        "whole numbers" => Some("только целые"),
        "number" => Some("число"),
        "Range" => Some("Диапазон"),
        "higher is better" => Some("чем больше, тем лучше"),
        "lower is better" => Some("чем меньше, тем лучше"),
        "best near" => Some("лучше всего около"),
        "Healthy" => Some("Норма"),
        "Limb list" => Some("Список конечностей"),
        "Order in which the save stores limbs (body.limbs)." => {
            Some("Порядок, в котором сохранение хранит конечности (body.limbs).")
        }
        "Options" => Some("Варианты"),
        "Default" => Some("По умолчанию"),
        "On" => Some("Вкл"),
        "Off" => Some("Выкл"),
        "Settings" => Some("Настройки"),
        "standard values (see World settings)" => Some("стандартные значения (см. Настройки мира)"),
        "Layer modifier" => Some("Модификатор слоя"),
        "Reached" => Some("Пройден"),
        "Not reached yet" => Some("Ещё не пройден"),
        "Not reached in the game yet: descend to this layer in a run first." => {
            Some("В игре ещё не достигнут: сначала спуститесь на этот слой в забеге.")
        }
        "Deepest layer reached: {layer}" => Some("Самый глубокий пройденный слой: {layer}"),
        "Layer {number}" => Some("Слой {number}"),
        "Armor" => Some("Броня"),
        "Insulation" => Some("Теплоизоляция"),
        "Container capacity" => Some("Вместимость контейнера"),
        "Max item weight" => Some("Макс. вес предмета"),
        "Liquid capacity" => Some("Объём жидкости"),
        "Worn on" => Some("Надевается на"),
        "Holding" => Some("Удержание"),
        "hands only" => Some("только в руках"),

        _ => None,
    }
}
