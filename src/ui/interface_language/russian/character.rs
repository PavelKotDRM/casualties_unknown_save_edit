pub(super) fn translate(source: &str) -> Option<&'static str> {
    match source {
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
        "This save has no `limbs` array." => Some("В сохранении отсутствует массив `limbs`."),
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
        "Starting skills:" => Some("Стартовые навыки:"),
        "Dossier and quirks" => Some("Досье и особенности"),
        "Unlocked characters" => Some("Открытые персонажи"),
        "Refresh" => Some("Обновить"),
        "Read the unlocks from the game again" => Some("Заново прочитать открытия из игры"),
        "Stored in the game's settings, not in the save. Character N unlocks after a run is won as character N − 1. Close the game before changing them." => {
            Some(
                "Хранится в настройках игры, а не в сохранении. Персонаж N открывается после победы в забеге за персонажа N − 1. Закройте игру перед изменением.",
            )
        }
        "Could not read the unlocks:" => Some("Не удалось прочитать открытия:"),
        "Always unlocked" => Some("Всегда открыт"),
        "Could not change the unlock:" => Some("Не удалось изменить открытие:"),
        "Locked in the game" => Some("Закрыт в игре"),
        "Reset skills to starting values" => Some("Сбросить навыки к стартовым значениям"),
        "Set the levels and XP gain multipliers this character starts a run with." => {
            Some("Установить уровни и множители опыта, с которыми этот персонаж начинает забег.")
        }
        "Character #{index}" => Some("Персонаж #{index}"),
        "Finish a run with the previous character to unlock" => {
            Some("Пройдите забег за предыдущего персонажа, чтобы открыть")
        }

        _ => None,
    }
}
