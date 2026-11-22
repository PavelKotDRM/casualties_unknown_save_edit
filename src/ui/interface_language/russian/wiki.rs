pub(super) fn translate(source: &str) -> Option<&'static str> {
    match source {
        "Animal corpse" => Some("Труп животного"),
        "Arms" => Some("Руки"),
        "Back (wear slot)" => Some("Спина"),
        "Balaclava" => Some("Балаклава"),
        "Back of thigh" => Some("Задняя часть бедра"),
        "Banana plant" => Some("Банановое растение"),
        "Bandolier" => Some("Патронташ"),
        "Barbed wire fence" => Some("Забор из колючей проволоки"),
        "Bear trap" => Some("Медвежий капкан"),
        "Belt" => Some("Пояс"),
        "Bio terminal" => Some("Биотерминал"),
        "Blindfold" => Some("Повязка на глаза"),
        "Blood crystal" => Some("Кровавый кристалл"),
        "Bounce cap" => Some("Прыгучий гриб"),
        "Brown shroom" => Some("Коричневый гриб"),
        "Cactus" => Some("Кактус"),
        "Cave tick" => Some("Пещерный клещ"),
        "Ceiling rye" => Some("Потолочная рожь"),
        "Ceiling spice vine" => Some("Потолочная пряная лиана"),
        "Ceiling turret" => Some("Потолочная турель"),
        "Slim ceiling turret" => Some("Узкая потолочная турель"),
        "Coil" => Some("Катушка"),
        "Container" => Some("Контейнер"),
        "Cold puff" => Some("Холодный пух"),
        "Container crate" => Some("Ящик-контейнер"),
        "Core crystal" => Some("Кристалл ядра"),
        "Corpse" => Some("Труп"),
        "Crimson bush" => Some("Багровый куст"),
        "Crimson tree" => Some("Багровое дерево"),
        "Crystal enemy" => Some("Кристаллический враг"),
        "Digestion crystal" => Some("Пищеварительный кристалл"),
        "Drug" => Some("Препарат"),
        "Drill pod" => Some("Буровая капсула"),
        "Broken drill pod" => Some("Сломанная буровая капсула"),
        "Drop capsule" => Some("Капсула сброса"),
        "Dry bush" => Some("Сухой куст"),
        "Emissive crystal" => Some("Излучающий кристалл"),
        "Eyes" => Some("Глаза"),
        "Fan" => Some("Вентилятор"),
        "Feet" => Some("Ступни"),
        "Food box" => Some("Ящик с едой"),
        "Frigiant" => Some("Фригиант"),
        "Fungalibungali" => Some("Фунгалингуали"),
        "Geo tree" => Some("Геодерево"),
        "Glow plant" => Some("Светящееся растение"),
        "Glow shroom" => Some("Светящийся гриб"),
        "Grabber plant" => Some("Растение-хвататель"),
        "Grabber shroom" => Some("Гриб-хвататель"),
        "Grate" => Some("Решётка"),
        "Gun mine" => Some("Мина с пушкой"),
        "Hands" => Some("Кисти рук"),
        "Hat" => Some("Головной убор"),
        "Heat fern" => Some("Тепловой папоротник"),
        "Hydreed" => Some("Гидрид"),
        "Jump pad" => Some("Прыжковая площадка"),
        "Knees" => Some("Колени"),
        "Ladder" => Some("Лестница"),
        "Land mine" => Some("Наземная мина"),
        "Laser drone" => Some("Лазерный дрон"),
        "Lava lamp" => Some("Лава-лампа"),
        "Lead bush" => Some("Свинцовый куст"),
        "Life pod chest" => Some("Сундук спасательной капсулы"),
        "Life pod light" => Some("Свет спасательной капсулы"),
        "Life pod pump" => Some("Насос спасательной капсулы"),
        "Life pod shower" => Some("Душ спасательной капсулы"),
        "Medical crate" => Some("Медицинский ящик"),
        "Mushlight" => Some("Мушлайт"),
        "Mushroom platform" => Some("Грибная платформа"),
        "Mushroom tree" => Some("Грибное дерево"),
        "Neck" => Some("Шея"),
        "Outer torso" => Some("Наружный торс"),
        "Overgrown tick" => Some("Разросшийся клещ"),
        "Oxygen crystal" => Some("Кислородный кристалл"),
        "Piston" => Some("Поршень"),
        "Left piston" => Some("Левый поршень"),
        "Right piston" => Some("Правый поршень"),
        "Pop" => Some("Поп"),
        "Pussack" => Some("Пуссак"),
        "Radiation barrel" => Some("Радиационная бочка"),
        "Relief crystal" => Some("Кристалл облегчения"),
        "Sand rose" => Some("Песчаная роза"),
        "Saw blade" => Some("Пильный диск"),
        "Scrapeater" => Some("Скрейпитер"),
        "Shade crawler" => Some("Теневой ползун"),
        "Side stabber" => Some("Боковой пронзатель"),
        "Flipped side stabber" => Some("Перевёрнутый боковой пронзатель"),
        "Skull crusher" => Some("Сокрушитель черепов"),
        "Snow strider" => Some("Снежный странник"),
        "Soothing crystal" => Some("Успокаивающий кристалл"),
        "Sound cannon" => Some("Звуковая пушка"),
        "Space heater" => Some("Обогреватель"),
        "Spice vine" => Some("Пряная лиана"),
        "Spike stabber" => Some("Шипастый пронзатель"),
        "Stone plant" => Some("Каменное растение"),
        "Tech ball" => Some("Техномяч"),
        "Thigh" => Some("Бедро"),
        "Elder thornback" => Some("Старый шипоспин"),
        "Young thornback" => Some("Молодой шипоспин"),
        "Tool" => Some("Инструмент"),
        "Torso" => Some("Торс"),
        "Front of torso" => Some("Передняя часть торса"),
        "Trash" => Some("Мусор"),
        "Trader 1" => Some("Торговец 1"),
        "Trader 2" => Some("Торговец 2"),
        "Trader 3" => Some("Торговец 3"),
        "Turbulent crystal" => Some("Бурный кристалл"),
        "Turret" => Some("Турель"),
        "Utility" => Some("Снаряжение"),
        "Water" => Some("Вода"),
        "Wearable" => Some("Носимое"),
        "Weapon" => Some("Оружие"),
        "Wall biter" => Some("Стеногрыз"),
        "Wraps" => Some("Обмотки"),
        "Game wiki" => Some("Вики игры"),
        "Status colours unavailable:" => Some("Цвета состояний недоступны:"),
        "Some game item images could not be loaded." => {
            Some("Не удалось загрузить некоторые изображения предметов из игры.")
        }
        "Search all sections" => Some("Искать во всех разделах"),
        "Search all sections…" => Some("Поиск по всем разделам…"),
        "Search this section…" => Some("Поиск в этом разделе…"),
        "Show matching items or liquids" => Some("Показать подходящие предметы или жидкости"),
        "Matching crafting quality: {quality} ≥{amount}" => {
            Some("Требуемое качество для создания: {quality} ≥{amount}")
        }
        "Matching liquid crafting quality: {quality} ≥{amount}" => {
            Some("Требуемое качество жидкости для создания: {quality} ≥{amount}")
        }
        "Clear filter" => Some("Сбросить фильтр"),
        "Sort by" => Some("Сортировать по"),
        "Name" => Some("Название"),
        "Wiki group" => Some("Группа вики"),
        "Crafting quality" => Some("Качество для создания"),
        "Drop chance" => Some("Шанс выпадения"),
        "Ascending" => Some("По возрастанию"),
        "Descending" => Some("По убыванию"),
        "Drop chance sort uses the highest per-source probability, including calculated random category-pool odds." => {
            Some(
                "Сортировка учитывает наибольшую вероятность для источника, включая рассчитанный шанс из случайных пулов категорий.",
            )
        }
        "overall chance" => Some("общая вероятность"),
        "Explicit rolls" => Some("Явные броски"),
        "picks" => Some("выборов"),
        "categories" => Some("категорий"),
        "eligible items" => Some("предметов в пуле"),
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
        "🛠 Crafting" => Some("🛠 Создание"),
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
        "Items known to the game with their stats from the installation. Drop probabilities combine explicit rolls, unconditional drops, and calculated category-pool selections. Each pool draw chooses a listed category and then an item uniformly, with replacement. Effects are read from the game's code: amounts are per use unless marked otherwise." => {
            Some(
                "Известные игре предметы с характеристиками из установленной игры. Вероятность выпадения объединяет явные шансы, безусловные выпадения и рассчитанные выборы из пулов категорий. Каждый выбор случайно выбирает категорию, затем предмет из неё; повторы возможны. Эффекты прочитаны из кода игры: значения указаны за одно использование, если не отмечено иное.",
            )
        }
        "Drop sources" => Some("Источники добычи"),
        "Merchant stock (Normal preset)" => Some("Товары торговца (стандартный пресет)"),
        "if destroyed" => Some("при уничтожении"),
        "guaranteed" => Some("гарантированно"),
        "at least one" => Some("вероятность хотя бы одного выпадения"),
        "Random category pool" => Some("Случайный пул категории"),
        "guaranteed random drops" => Some("гарантированных случайных предметов"),
        "Recipes, crafting trees, requirements and special crafting rules." => {
            Some("Рецепты, деревья создания, требования и особые правила.")
        }
        "Crafting rules" => Some("Правила создания"),
        "Recipe visibility" => Some("Отображение рецептов"),
        "Unlearned recipes appear when INT is no more than 3 below the requirement; specially known recipes always appear." => {
            Some(
                "Неизученные рецепты отображаются, если ИНТ отстаёт от требования не более чем на 3; особые изученные рецепты отображаются всегда.",
            )
        }
        "Ingredients" => Some("Ингредиенты"),
        "Crafting tree" => Some("Дерево создания"),
        "Examples" => Some("Примеры"),
        "more" => Some("ещё"),
        "Deeper recipes omitted" => Some("Более глубокие рецепты скрыты"),
        "Ingredients can come from inventory or nearby ground items within 10 units. Favourited items and non-empty containers are excluded." => {
            Some(
                "Ингредиенты можно брать из инвентаря или с земли в радиусе 10 единиц. Избранные предметы и непустые контейнеры не используются.",
            )
        }
        "Low INT" => Some("Недостаточный ИНТ"),
        "At 1 INT below the requirement, half of crafts produce a result at 20–90% of its listed condition or volume. At 2 or more below, half produce nothing; at exactly 3 below, that failed attempt also injures the player." => {
            Some(
                "При отставании на 1 ИНТ половина попыток даёт результат с 20–90% указанного состояния или объёма. При отставании на 2 и более половина попыток не даёт результата; ровно при −3 ИНТ неудачная попытка также ранит игрока.",
            )
        }
        "Ingredient use" => Some("Расход ингредиентов"),
        "A craft uses its ingredients before the result is rolled. Specific items lose the listed condition amount; quality ingredients lose condition in proportion to the quality supplied. Ingredients marked consumed are destroyed; liquid ingredients are drained." => {
            Some(
                "Ингредиенты расходуются до проверки результата. Состояние конкретных предметов уменьшается на указанную величину; у предметов с качеством состояние уменьшается пропорционально затраченному качеству. Расходуемые предметы уничтожаются, жидкие ингредиенты сливаются.",
            )
        }
        "First craft" => Some("Первое создание"),
        "Container results" => Some("Результаты-контейнеры"),
        "A crafted liquid container is emptied and set to 0 condition unless its recipe preserves the result liquid." => {
            Some(
                "Созданный контейнер для жидкости опустошается и получает 0 состояния, если рецепт не сохраняет жидкость.",
            )
        }
        "Making a recipe for the first time raises happiness and grants INT experience." => {
            Some("Первое создание по рецепту повышает настроение и даёт опыт ИНТ.")
        }
        "Materials" => Some("Материалы"),
        "Tools" => Some("Инструменты"),
        "Medicine" => Some("Медицина"),
        "Utilities" => Some("Полезные предметы"),
        "Food" => Some("Еда"),
        "Repair" => Some("Ремонт"),
        "Required volume" => Some("Необходимый объём"),
        "INT" => Some("ИНТ"),
        "Category" => Some("Категория"),
        "INT required" => Some("Требуется ИНТ"),
        "Produces" => Some("Результат"),
        "Specially known" => Some("Особый рецепт"),
        "Keeps result liquid" => Some("Сохраняет жидкость"),
        "Any item with" => Some("Любой предмет с качеством"),
        "Any liquid with" => Some("Любая жидкость с качеством"),
        "quality amount" => Some("уровень качества"),
        "condition" => Some("состояние"),
        "kept" => Some("сохраняется"),
        "consumed" => Some("расходуется"),
        "not" => Some("кроме"),
        "🧪 Liquids" => Some("🧪 Жидкости"),
        "Liquids that containers can hold, with the effects of drinking or injecting 100 ml, read from the game's code." => {
            Some(
                "Жидкости, которые могут храниться в ёмкостях, с эффектами от 100 мл при питье или инъекции, прочитанными из кода игры.",
            )
        }
        "Ctrl+F to search, Esc to clear. Every word must match. Search is limited to the selected section unless Search all sections is enabled. Crafting-quality queries include matching results from all sections." => {
            Some(
                "Ctrl+F — поиск, Esc — очистить. Должны совпасть все слова. Поиск ограничен выбранным разделом, если не включён поиск по всем разделам. Запросы по качеству для создания показывают совпадения из всех разделов.",
            )
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
