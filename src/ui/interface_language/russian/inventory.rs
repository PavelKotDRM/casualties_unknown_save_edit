pub(super) fn translate(source: &str) -> Option<&'static str> {
    match source {
        "Minimum carrying load" => Some("Минимальная переносимая нагрузка"),
        "Equal stored weight" => Some("Одинаковый вес содержимого"),
        "Equal fill percentage" => Some("Одинаковый процент заполнения"),
        "Move favourites" => Some("Перемещать избранное"),
        "Balance containers" => Some("Сбалансировать контейнеры"),
        "Repair & charge all" => Some("Починить и зарядить всё"),
        "Repairs items and fully charges installed batteries. Liquid levels are unchanged." => {
            Some(
                "Чинит предметы и полностью заряжает установленные батареи. Уровень жидкости не меняется.",
            )
        }
        "{count} items moved" => Some("Перемещено предметов: {count}"),
        "Equipped items and compatible contents of specialized containers stay in place. Redistribute the remaining items only. If an optimum cannot be proven within the search limit, nothing is changed." => {
            Some(
                "Экипировка и подходящее содержимое специализированных контейнеров остаются на месте. Перераспределяются только остальные предметы. Если оптимальность не доказана до лимита поиска, ничего не меняется.",
            )
        }
        "Ammunition" => Some("Боеприпасы"),
        "Rounds" => Some("Патроны"),
        "Chamber" => Some("Патронник"),
        "Live round" => Some("Боевой патрон"),
        "Spent casing" => Some("Стреляная гильза"),
        "No round" => Some("Нет патрона"),
        "No magazine inserted" => Some("Магазин не вставлен"),
        "Reload from inventory" => Some("Перезарядить из инвентаря"),
        "Select a magazine or weapon to reload it." => {
            Some("Для перезарядки выберите магазин или оружие.")
        }
        "Uses compatible non-favourite ammunition. Inserts a magazine when needed and chambers a round. Safety is unchanged." => {
            Some(
                "Использует совместимые боеприпасы не из избранного. При необходимости вставляет магазин и досылает патрон. Предохранитель не меняется.",
            )
        }
        "Already loaded, or no compatible non-favourite ammunition is available" => {
            Some("Уже заряжено, либо нет совместимых боеприпасов вне избранного")
        }
        "At least two equipped containers are needed" => {
            Some("Нужно хотя бы два экипированных контейнера")
        }
        "Balancing search limit reached. No items were moved; pin more items as favourites or choose another rule." => {
            Some(
                "Достигнут лимит поиска. Предметы не перемещены; закрепите больше предметов в избранном или выберите другое правило.",
            )
        }
        "The first item of a location is held / worn; the following ones are stored inside it." => {
            Some("Первый предмет в ячейке удерживается или надет; остальные хранятся внутри него.")
        }
        "The first item in this slot is its holder; any following items are stored inside it." => {
            Some(
                "Первый предмет в этом слоте удерживается персонажем; остальные хранятся внутри него.",
            )
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
        "Includes the game's default liquids, batteries and component state. Edit contents after adding." => {
            Some(
                "Добавляются жидкости, батареи и состояние компонентов по умолчанию из игры. Содержимое можно изменить после добавления.",
            )
        }
        "Location" => Some("Расположение"),
        "Unknown slot {index}" => Some("Неизвестный слот {index}"),
        "Mouth" => Some("Рот"),
        "Upper back" => Some("Верх спины"),
        "Middle back" => Some("Середина спины"),
        "Lower back" => Some("Низ спины"),
        "Item cannot be worn here" => Some("Предмет нельзя надеть сюда"),
        "Can only be held in hands" => Some("Можно держать только в руках"),
        "Clothing can't be held; wear it or store it in a container" => {
            Some("Экипировку нельзя держать; наденьте её или уберите в контейнер")
        }
        "⚠ The game doesn't let clothing be held; wear it or store it in a container." => {
            Some("⚠ Игра не позволяет держать экипировку; наденьте её или уберите в контейнер.")
        }
        "⚠ The game only lets this item be held in hands; move it to a hand slot." => {
            Some("⚠ Игра позволяет держать этот предмет только в руках; переместите его в руку.")
        }
        "Worn" => Some("Надето"),
        "Unassigned (slot -1, no wear slot)" => Some("Не назначено (нет слота одежды)"),
        "Select an item to edit it." => Some("Выберите предмет для редактирования."),
        "The inventory is empty." => Some("Инвентарь пуст."),
        "Item id" => Some("ID предмета"),
        "Favourited" => Some("Избранное"),
        "Label" => Some("Метка"),
        "Swatch" => Some("Вариант цвета"),
        "Colour variant index" => Some("Номер варианта цвета"),
        "Cannot be worn: {limb} is dismembered" => {
            Some("Нельзя надеть: {limb} — конечность отсечена")
        }
        "A container with items can't be stored in another container" => {
            Some("Контейнер с предметами нельзя положить в другой контейнер")
        }
        "⚠ {limb} is dismembered: the game won't wear this item and drops it on load." => Some(
            "⚠ {limb} — конечность отсечена: игра не наденет предмет и бросит его при загрузке.",
        ),
        "⚠ The game has no such slot: loading fails for this item and shifts the saved component data of later items." => {
            Some(
                "⚠ В игре нет такого слота: предмет не загрузится, а данные компонентов следующих предметов сместятся.",
            )
        }
        "⚠ Condition 0: the game destroys this item (a container spills its contents)." => {
            Some("⚠ Состояние 0: игра уничтожит предмет (содержимое контейнера высыплется).")
        }
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
        "Charge" => Some("Заряд"),
        "No battery" => Some("Нет батареи"),
        "Uses remaining" => Some("Осталось использований"),
        "Uses remaining (limb)" => Some("Осталось применений на конечность"),
        "Calculated from the game's fixed condition cost and threshold. Other action requirements still apply." => {
            Some(
                "Рассчитывается по расходу состояния и порогу из игры. Остальные требования действия по-прежнему применяются.",
            )
        }
        "Condition is calculated from contents. Edit the contents below." => {
            Some("Состояние рассчитывается по содержимому. Измените содержимое ниже.")
        }
        "Saved state does not match the contents." => {
            Some("Сохранённое состояние не соответствует содержимому.")
        }
        "Synchronize with contents" => Some("Синхронизировать с содержимым"),
        "Invalid item components" => Some("Некорректные компоненты предмета"),
        "Invalid item object" => Some("Некорректный объект предмета"),
        "Invalid battery type" => Some("Некорректный тип батареи"),
        "Invalid item condition" => Some("Некорректное состояние предмета"),
        "Invalid item id" => Some("Некорректный ID предмета"),
        "Invalid battery component" => Some("Некорректный компонент батареи"),
        "Invalid liquid stack" => Some("Некорректный список жидкостей"),
        "Invalid liquid amount" => Some("Некорректное количество жидкости"),
        "Liquid total is not finite" => Some("Общий объём жидкости не является конечным числом"),
        "Liquid condition is not finite" => Some("Состояние жидкости не является конечным числом"),
        "Battery charge must be between zero and its maximum" => {
            Some("Заряд батареи должен быть между нулём и максимумом")
        }
        "Invalid condition or item-use rule" => {
            Some("Некорректное состояние или правило использования предмета")
        }
        "Item-use count exceeds the calculation limit" => {
            Some("Число использований превышает предел расчёта")
        }
        "Battery compatibility for {id} is unavailable" => {
            Some("Совместимость батарей для {id} неизвестна")
        }
        "Battery" => Some("Батарея"),
        "Remove this component (use defaults)" => {
            Some("Удалить компонент (использовать значения по умолчанию)")
        }
        "Fill to capacity" => Some("Заполнить до максимума"),
        "Scales all liquids proportionally" => {
            Some("Пропорционально изменяет количество всех жидкостей")
        }
        "Category:" => Some("Категория:"),
        "Game category:" => Some("Категория в игре:"),
        "Game category" => Some("Категория в игре"),
        "Weapons & ammo" => Some("Оружие и боеприпасы"),
        "Tools & melee weapons" => Some("Инструменты и холодное оружие"),
        "Clothing & armour" => Some("Одежда и броня"),
        "Bags & containers" => Some("Сумки и контейнеры"),
        "Medical" => Some("Медицина"),
        "Drugs" => Some("Лекарства"),
        "Food" => Some("Еда"),
        "Drinks" => Some("Напитки"),
        "Devices & gear" => Some("Устройства и снаряжение"),
        "Materials" => Some("Материалы"),
        "Unobtainable" => Some("Недоступные"),
        "Search name or id" => Some("Поиск по названию или ID"),
        "All categories" => Some("Все категории"),
        "Nothing found." => Some("Ничего не найдено."),
        "Put into" => Some("Поместить в"),
        "New item will be held/worn" => Some("Новый предмет будет в руках или надет."),
        "Current location" => Some("Текущее расположение"),
        "Empty location" => Some("Свободное расположение"),
        "Item data unavailable" => Some("Данные предмета недоступны."),
        "Container data unavailable" => Some("Данные контейнера недоступны."),
        "Inside {item}: capacity unavailable" => Some("Внутри «{item}»: вместимость неизвестна"),
        "Inside {item}: weight limits unavailable" => {
            Some("Внутри «{item}»: ограничения веса неизвестны")
        }
        "Cannot verify item weight for {item}" => {
            Some("Невозможно проверить вес предмета для «{item}»")
        }
        "Item exceeds max weight per item for {item}" => {
            Some("Вес предмета превышает лимит контейнера «{item}»")
        }
        "Not enough remaining capacity in {item}" => {
            Some("В контейнере «{item}» недостаточно свободного места")
        }
        "Inside {item}: restrictions unavailable" => {
            Some("Внутри «{item}»: ограничения неизвестны")
        }
        "Cannot check item type for {item}" => {
            Some("Невозможно проверить тип предмета для «{item}»")
        }
        "Item type not accepted by {item}" => {
            Some("Контейнер «{item}» не принимает этот тип предмета")
        }
        "Inside {item}: full" => Some("Внутри «{item}»: места нет"),
        "Inside {item}: over capacity" => Some("Внутри «{item}»: превышена вместимость"),
        "Inside {item}: {remaining}/{capacity} u left" => {
            Some("Внутри «{item}»: осталось {remaining}/{capacity} u")
        }
        "Occupied by {item} (not a container)" => Some("Занято предметом «{item}» (не контейнер)"),
        "Container data unavailable for {item}" => Some("Данные контейнера «{item}» недоступны"),
        "Space left: {weight} u" => Some("Свободно: {weight} u"),
        "Full" => Some("Места нет"),
        "Over capacity by {weight} u" => Some("Превышена вместимость на {weight} u"),
        "Container capacity unavailable" => Some("Вместимость контейнера недоступна"),
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
