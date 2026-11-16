/// An ending of a run, recorded by the game in its preferences once reached.
#[derive(Debug, Clone, Copy)]
pub struct EndingDef {
    /// `PlayerPrefs` key set to `1` when the ending is reached
    /// (also restored on start from the Steam achievement).
    pub pref_key: &'static str,
    /// Steam achievement unlocked by the ending.
    pub achievement: &'static str,
    /// English name.
    pub name: &'static str,
    /// Locale key (`other` table) of the case status on the end screen.
    pub status_key: &'static str,
    /// English fallback of the end screen case status.
    pub status: &'static str,
    /// How the ending is reached.
    pub how: &'static str,
}

/// Endings in the order the game numbers its end screens (`EndSequence`
/// types 1–3). Ported from `WorldGeneration` / `EndLabScript`: add new
/// endings here and they are tracked everywhere.
pub const ENDINGS: [EndingDef; 3] = [
    EndingDef {
        pref_key: "didbiopod",
        achievement: "BIOPOD",
        name: "Biopod",
        status_key: "endscreenresolved",
        status: "RETRIEVED",
        how: "Reach the lab at the bottom of \"Where we must reach\" and follow the scientist to the exit. Letting your brain fail in the lab gives the same ending.",
    },
    EndingDef {
        pref_key: "didescape",
        achievement: "COMPANYSWORST",
        name: "Escape",
        status_key: "endscreenescape",
        status: "MORE INFO AT MEETING",
        how: "Get the explosive chip out of your head, bring a gun to the lab at the bottom of \"Where we must reach\", shoot the scientist and run for the exit.",
    },
    EndingDef {
        pref_key: "didmerc",
        achievement: "TOBECONTINUED",
        name: "To be continued",
        status_key: "endscreenfailure",
        status: "CATASTROPHIC FAILURE",
        how: "Available after the Biopod or Escape ending: start a mercenary run (first layer, chip in place, not endless), keep the mercenary scarf and climb the Crimson ascent to the surface.",
    },
];
