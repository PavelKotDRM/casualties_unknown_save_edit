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
        "Backups of" => Some("Копии файла"),
        "Select all" => Some("Выбрать все"),
        "Select none" => Some("Снять выбор"),
        "Delete selected ({count})" => Some("Удалить выбранные ({count})"),
        "Delete {count} backups permanently?" => Some("Удалить навсегда копий: {count}?"),
        "Old backups:" => Some("Старые копии:"),
        "Keep the newest" => Some("Оставить новейшие"),
        "Older than (days)" => Some("Старше (дней)"),
        "Select {count}" => Some("Выбрать ({count})"),
        "Check the backups matched by the rule (then delete them)" => {
            Some("Отметить копии, подходящие под правило (затем их можно удалить)")
        }
        "Refresh data automatically" => Some("Обновлять данные автоматически"),
        "Every few seconds re-read the save on disk, unlocks, achievements, backups and the game installation" => {
            Some(
                "Каждые несколько секунд перечитывать сохранение на диске, открытия, достижения, резервные копии и установку игры",
            )
        }
        "Refresh now" => Some("Обновить сейчас"),
        "Data refreshed" => Some("Данные обновлены"),
        "Achievements" => Some("Достижения"),
        "Steam achievements" => Some("Достижения Steam"),
        "All" => Some("Все"),
        "Search" => Some("Поиск"),
        "Nothing matches." => Some("Ничего не найдено."),
        "Hidden achievement" => Some("Скрытое достижение"),
        "Unlock it to see what it is." => Some("Получите его, чтобы узнать, какое оно."),
        "Unlocked {date}" => Some("Получено {date}"),
        "{found} of {total} unlocked" => Some("Получено {found} из {total}"),
        "Reading Steam's achievement data…" => Some("Чтение данных о достижениях Steam…"),
        "Steam's achievement data was not found:" => Some("Данные о достижениях Steam не найдены:"),
        "Achievements are read from Steam's local cache (appcache/stats); start the game through Steam once to create it." => {
            Some(
                "Достижения читаются из локального кэша Steam (appcache/stats); запустите игру через Steam хотя бы раз, чтобы он появился.",
            )
        }
        "No unlocked achievements were found for this computer's Steam account; all are shown as locked." => {
            Some(
                "Для аккаунта Steam на этом компьютере не найдено полученных достижений; все показаны как закрытые.",
            )
        }
        "Read from Steam (read-only); refreshed automatically when Steam updates it." => Some(
            "Данные Steam (только чтение); обновляются автоматически, когда их обновляет Steam.",
        ),
        "Report a problem" => Some("Сообщить о проблеме"),
        "Report a problem…" => Some("Сообщить о проблеме…"),
        "Report this problem…" => Some("Сообщить об этой проблеме…"),
        "🔒 Nothing is sent automatically. Review the text below: it is exactly what will be shared. Your user and computer names, home folder, Steam account and e-mail addresses are replaced with placeholders, and save contents are never included." => {
            Some(
                "🔒 Ничего не отправляется автоматически. Проверьте текст ниже: именно он будет передан. Имена пользователя и компьютера, домашняя папка, аккаунт Steam и адреса e-mail заменены метками, а содержимое сохранений никогда не включается.",
            )
        }
        "Possible problems found" => Some("Найдены возможные проблемы"),
        "Use as title" => Some("Сделать заголовком"),
        "Title" => Some("Заголовок"),
        "Short summary of the problem" => Some("Кратко о проблеме"),
        "What happened?" => Some("Что произошло?"),
        "What did you do, what did you expect, and what happened instead?" => {
            Some("Что вы делали, что ожидали и что произошло на самом деле?")
        }
        "Include:" => Some("Включить:"),
        "Recent errors" => Some("Недавние ошибки"),
        "Last crash" => Some("Последний сбой"),
        "Game progress and save summary" => Some("Прогресс игры и сводку сохранения"),
        "Unlocked characters, endings and achievements count, and the character, layer and run time of the open save" => {
            Some(
                "Число открытых персонажей, концовок и достижений, а также персонаж, слой и время забега открытого сохранения",
            )
        }
        "Debugging information" => Some("Отладочная информация"),
        "Regenerate" => Some("Собрать заново"),
        "Discard your edits to this text" => Some("Отменить ваши правки этого текста"),
        "Create GitHub issue…" => Some("Создать issue на GitHub…"),
        "Opens a pre-filled issue in your browser; you can still edit it there before submitting. Needs a GitHub account." => {
            Some(
                "Открывает заполненный issue в браузере; перед отправкой его можно отредактировать. Нужен аккаунт GitHub.",
            )
        }
        "Enter a title first" => Some("Сначала введите заголовок"),
        "Copy" => Some("Копировать"),
        "Save to file…" => Some("Сохранить в файл…"),
        "Delete crash reports" => Some("Удалить отчёты о сбоях"),
        "The editor crashed" => Some("Редактор аварийно завершился"),
        "Unsupported game version" => Some("Неподдерживаемая версия игры"),
        "Item stats could not be read from the game" => {
            Some("Не удалось прочитать характеристики предметов из игры")
        }
        "The game's preferences could not be read" => Some("Не удалось прочитать настройки игры"),
        "The game's preferences could not be changed" => Some("Не удалось изменить настройки игры"),
        "Steam achievement data was not read" => Some("Данные о достижениях Steam не прочитаны"),
        "Save folder not found" => Some("Папка сохранений не найдена"),
        "{count} backups" => Some("Копий: {count}"),
        "The save folder was not found." => Some("Папка сохранений не найдена."),
        "Select a backup to see what it contains." => {
            Some("Выберите копию, чтобы увидеть её содержимое.")
        }
        "The backup cannot be read" => Some("Не удалось прочитать копию"),
        "The current save cannot be read" => Some("Не удалось прочитать текущее сохранение"),
        "Delete permanently" => Some("Удалить навсегда"),
        "Backup" => Some("Копия"),
        "Current save" => Some("Текущее сохранение"),
        "Run time" => Some("Время забега"),
        "Distance" => Some("Пройдено"),
        "m" => Some("м"),
        "Items" => Some("Предметы"),
        "Blood" => Some("Кровь"),
        "yes" => Some("да"),
        "unknown date" => Some("дата неизвестна"),
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
        "Character height. The game rolls it from a range scaled by the character's size." => Some(
            "Рост персонажа. Игра выбирает его случайно из диапазона, масштабированного по размеру персонажа.",
        ),
        "game range:" => Some("в игре:"),
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
