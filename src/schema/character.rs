/// Names of the playable characters (`character` field).
pub const CHARACTERS: &[&str] = &["Experiment", "Character #1", "Character #2"];

/// The three skills stored in `body.skills`.
#[derive(Debug, Clone, Copy)]
pub struct SkillDef {
    /// Key prefix (`STR`, `RES`, `INT`).
    pub key: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Locale key of the name in the `other` table.
    pub locale_key: &'static str,
    /// Short description.
    pub help: &'static str,
}

/// Strength, resilience and intelligence.
pub const SKILLS: [SkillDef; 3] = [
    SkillDef {
        key: "STR",
        name: "Strength",
        locale_key: "strength",
        help: "Attack damage, throw strength, hand speed in minigames, carry capacity.",
    },
    SkillDef {
        key: "RES",
        name: "Resilience",
        locale_key: "resilience",
        help: "Resistance to damage, pain and illness; carry capacity.",
    },
    SkillDef {
        key: "INT",
        name: "Intelligence",
        locale_key: "intelligence",
        help: "Crafting recipes, reading notes and using complex items.",
    },
];

/// Total experience required to reach `level`
/// (port of `Skills.GetExperienceForLevel` from the game).
pub fn experience_for_level(level: i32) -> i64 {
    if level <= 0 {
        return 0;
    }
    let mut total: f32 = 0.0;
    let mut step: f32 = 60.0;
    for i in 1..=level {
        if i < 10 {
            total += 60.0;
            continue;
        }
        if i == 10 {
            step = 100.0;
            total += step;
            continue;
        }
        step *= 1.18;
        if i > 20 {
            step *= 1.15;
            if i >= 30 {
                step *= 2.0;
            }
        }
        total += step;
    }
    // Mathf.RoundToInt rounds half to even.
    total.round_ties_even() as i64
}

/// Display name of a world layer (`biome` stores the layer index the run
/// resumes on; titles are `layertitle<biome + 1>` in the locale).
pub const LAYERS: &[&str] = &[
    "Gravel lands",
    "Deeper gravel lands",
    "Dried desert",
    "Wasteland",
    "Overgrown depths",
    "Frozen chasm",
    "Fungal pools",
    "Crystalline hollow",
    "Geothermal vents",
    "Where we must reach",
    "Crimson ascent",
];
