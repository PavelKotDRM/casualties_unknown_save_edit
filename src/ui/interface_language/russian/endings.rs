pub(super) fn translate(source: &str) -> Option<&'static str> {
    match source {
        "Endings" => Some("Концовки"),
        "🏁 Endings" => Some("🏁 Концовки"),
        "Ending #{number}" => Some("Концовка №{number}"),
        "{found} of {total} discovered" => Some("открыто {found} из {total}"),
        "Not discovered yet" => Some("Ещё не открыта"),
        "Not discovered yet." => Some("Ещё не открыта."),
        "Discovered" => Some("Открыта"),
        "Steam achievement" => Some("Достижение Steam"),
        "The game marks an ending again on start if you have its Steam achievement." => {
            Some("Если у вас есть достижение Steam за концовку, игра снова отметит её при запуске.")
        }
        "Ways a run can end successfully. An ending stays hidden until you reach it in the game." => {
            Some(
                "Способы успешно завершить забег. Концовка скрыта, пока вы не дойдёте до неё в игре.",
            )
        }
        "Biopod" => Some("Биокапсула"),
        "Escape" => Some("Побег"),
        "To be continued" => Some("Продолжение следует"),
        "Reach the lab at the bottom of \"Where we must reach\" and follow the scientist to the exit. Letting your brain fail in the lab gives the same ending." => {
            Some(
                "Доберитесь до лаборатории на дне слоя «Куда нам нужно добраться» и пройдите за учёным к выходу. Если в лаборатории откажет мозг, концовка будет той же.",
            )
        }
        "Get the explosive chip out of your head, bring a gun to the lab at the bottom of \"Where we must reach\", shoot the scientist and run for the exit." => {
            Some(
                "Избавьтесь от взрывного чипа в голове, принесите оружие в лабораторию на дне слоя «Куда нам нужно добраться», застрелите учёного и бегите к выходу.",
            )
        }
        "Available after the Biopod or Escape ending: start a mercenary run (first layer, chip in place, not endless), keep the mercenary scarf and climb the Crimson ascent to the surface." => {
            Some(
                "Доступна после концовки «Биокапсула» или «Побег»: начните забег наёмника (первый слой, чип на месте, не бесконечный режим), сохраните шарф M.E.R.C. и поднимитесь по Багряному восхождению на поверхность.",
            )
        }
        _ => None,
    }
}
