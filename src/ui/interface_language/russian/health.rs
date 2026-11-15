pub(super) fn translate(source: &str) -> Option<&'static str> {
    match source {
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

        _ => None,
    }
}
