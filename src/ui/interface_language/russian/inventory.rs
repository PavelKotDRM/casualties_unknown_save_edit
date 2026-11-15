pub(super) fn translate(source: &str) -> Option<&'static str> {
    match source {
        "The first item of a location is held / worn; the following ones are stored inside it." => {
            Some("Первый предмет в ячейке удерживается или надет; остальные хранятся внутри него.")
        }
        "No saved component state (the game uses the item's defaults)." => {
            Some("Сохранённое состояние компонента отсутствует (используются значения игры).")
        }
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
            Some("⚠ Предмет предназначен для слота одежды «{expected}», но назначен в «{actual}».")
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
        "Item stats unavailable; sync game data to show verified weight." => {
            Some("Статистика предмета недоступна. Синхронизируйте данные игры, чтобы увидеть вес.")
        }
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
            Some("Ячейка занята: предмет будет помещён внутрь удерживаемого или надетого предмета.")
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
        "Click or drag to set the value" => Some("Нажмите или перетащите, чтобы изменить значение"),

        _ => None,
    }
}
