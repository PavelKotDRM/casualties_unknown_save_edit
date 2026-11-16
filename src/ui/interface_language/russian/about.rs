pub(super) fn translate(source: &str) -> Option<&'static str> {
    match source {
        "Game v{version}" => Some("Игра v{version}"),
        "Game version unknown" => Some("Версия игры неизвестна"),
        "Editor" => Some("Редактор"),
        "Editor version" => Some("Версия редактора"),
        "The editor's built-in data matches this game version." => {
            Some("Встроенные данные редактора соответствуют этой версии игры.")
        }
        "The editor was made for game version {supported}; version {installed} may add characters, endings or settings it does not know yet. Names, descriptions and item stats are still read from the installed game." => {
            Some(
                "Редактор создан для версии игры {supported}; в версии {installed} могут быть персонажи, концовки или настройки, которых он ещё не знает. Названия, описания и характеристики предметов по-прежнему читаются из установленной игры.",
            )
        }
        "Could not determine the game version." => Some("Не удалось определить версию игры."),
        "Made for game version" => Some("Создан для версии игры"),
        "Build" => Some("Сборка"),
        "Source code" => Some("Исходный код"),
        "Game" => Some("Игра"),
        "Game folder not found — set it in the Game data menu." => {
            Some("Папка игры не найдена — укажите её в меню «Данные игры».")
        }
        "Folder" => Some("Папка"),
        "Engine" => Some("Движок"),
        "Steam app" => Some("Приложение Steam"),
        "Steam build" => Some("Сборка Steam"),
        "Last updated" => Some("Последнее обновление"),
        "Size on disk" => Some("Размер на диске"),
        "Not a Steam installation." => Some("Установлена не через Steam."),
        "Progress" => Some("Прогресс"),
        "{found} of {total}" => Some("{found} из {total}"),
        "Characters unlocked" => Some("Открыто персонажей"),
        "Deepest layer reached" => Some("Самый глубокий слой"),
        "Endings discovered" => Some("Открыто концовок"),
        _ => None,
    }
}
