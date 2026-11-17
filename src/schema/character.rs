/// A playable character (`character` field / the game's `Body.charType`).
#[derive(Debug, Clone, Copy)]
pub struct CharacterDef {
    /// English fallback name (locale `char<index>name`).
    pub name: &'static str,
    /// Starting STR, RES and INT levels (`Skills.BaseSkills`).
    pub base_skills: [i64; 3],
    /// STR, RES and INT experience gain multipliers (`Skills.BaseMultipliers`).
    pub gain_multipliers: [f64; 3],
}

/// Playable characters in the game's order. Character `i > 0` is locked
/// until a run is won as character `i - 1` (see [`crate::player_prefs`]).
// FIXME: Character names, starting STR/RES/INT levels, and per-skill XP multipliers are hard-coded.
pub const CHARACTERS: [CharacterDef; 3] = [
    CharacterDef {
        name: "[SAW-01] \"Experiment\"",
        base_skills: [9, 8, 7],
        gain_multipliers: [1.0, 1.0, 1.0],
    },
    CharacterDef {
        name: "[SAW-03] \"Milky\"",
        base_skills: [4, 6, 12],
        gain_multipliers: [0.5, 0.5, 1.0],
    },
    CharacterDef {
        name: "[SAW-12] \"Dune\"",
        base_skills: [17, 18, 0],
        gain_multipliers: [1.0, 1.0, 0.5],
    },
];

/// A body slot that holds an item (`items[].slot >= 0`).
#[derive(Debug, Clone, Copy)]
pub struct HoldSlotDef {
    /// English fallback name.
    pub name: &'static str,
    /// Locale key of the name in the `other` table.
    pub locale_key: &'static str,
    /// Icon shown before the name.
    pub icon: &'static str,
}

/// The player's hold slots in `Body.slots` order, read from the game's body
/// prefab; labels match the inventory HUD.
// FIXME: The six slot names, locale keys, and their game-defined order are hard-coded.
pub const HOLD_SLOTS: [HoldSlotDef; 6] = [
    HoldSlotDef {
        name: "Right hand",
        locale_key: "HandF",
        icon: "✋",
    },
    HoldSlotDef {
        name: "Left hand",
        locale_key: "HandB",
        icon: "🤚",
    },
    HoldSlotDef {
        name: "Mouth",
        locale_key: "mouth",
        icon: "👄",
    },
    HoldSlotDef {
        name: "Upper back",
        locale_key: "upperback",
        icon: "🎒",
    },
    HoldSlotDef {
        name: "Middle back",
        locale_key: "middleback",
        icon: "🎒",
    },
    HoldSlotDef {
        name: "Lower back",
        locale_key: "lowerback",
        icon: "🎒",
    },
];

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
// FIXME: The game skill IDs, display names, locale keys, and help descriptions are hard-coded.
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
// FIXME: The XP curve is hard-coded: 60 XP per level below 10, 100 at level 10, then 1.18 growth, 1.15 after level 20, and a 2x multiplier from level 30.
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
// FIXME: All 11 layer names and their save-index order are hard-coded.
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
