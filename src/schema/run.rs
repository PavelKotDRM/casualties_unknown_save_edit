/// Kind of a run setting value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RunSettingKind {
    /// On/off.
    Bool,
    /// Bounded float.
    Float {
        /// Minimum.
        min: f32,
        /// Maximum.
        max: f32,
        /// Only whole numbers are allowed.
        whole: bool,
    },
    /// Choice among named options (stored as the option index).
    Dropdown(&'static [&'static str]),
}

/// Metadata of a run setting (`runSettings[].Item1`).
#[derive(Debug, Clone, Copy)]
pub struct RunSettingDef {
    /// Setting id.
    pub id: &'static str,
    /// English fallback label.
    pub label: &'static str,
    /// Value kind.
    pub kind: RunSettingKind,
    /// Unit suffix (`x`, `%`, `m`, `°C`).
    pub postfix: &'static str,
    /// Section in the game's run settings screen.
    pub section: &'static str,
}

const fn rs_bool(id: &'static str, label: &'static str, section: &'static str) -> RunSettingDef {
    RunSettingDef {
        id,
        label,
        kind: RunSettingKind::Bool,
        postfix: "",
        section,
    }
}

const fn rs_float(
    id: &'static str,
    label: &'static str,
    min: f32,
    max: f32,
    postfix: &'static str,
    section: &'static str,
) -> RunSettingDef {
    RunSettingDef {
        id,
        label,
        kind: RunSettingKind::Float {
            min,
            max,
            whole: false,
        },
        postfix,
        section,
    }
}

const fn rs_whole(
    id: &'static str,
    label: &'static str,
    min: f32,
    max: f32,
    postfix: &'static str,
    section: &'static str,
) -> RunSettingDef {
    RunSettingDef {
        id,
        label,
        kind: RunSettingKind::Float {
            min,
            max,
            whole: true,
        },
        postfix,
        section,
    }
}

/// All run settings known to the game, with the limits of the in-game sliders.
pub const RUN_SETTINGS: &[RunSettingDef] = &[
    rs_bool("unchipped", "Unchipped", "Run"),
    rs_bool("endless", "Endless", "Run"),
    rs_float(
        "baselootdensity",
        "Base loot density",
        0.0,
        2.0,
        "x",
        "Layer",
    ),
    rs_float(
        "lootmultiplier",
        "Loot multiplier per layer",
        0.0,
        1.0,
        "x",
        "Layer",
    ),
    rs_float(
        "basetrapdensity",
        "Base trap density",
        0.0,
        3.0,
        "x",
        "Layer",
    ),
    rs_float(
        "trapincrease",
        "Trap increase per layer",
        0.0,
        1.0,
        "x",
        "Layer",
    ),
    RunSettingDef {
        id: "ambientlight",
        label: "Ambient light",
        kind: RunSettingKind::Dropdown(&["None", "Enough", "Bright"]),
        postfix: "",
        section: "Layer",
    },
    rs_whole("timelimit", "Time limit", 5.0, 300.0, "m", "Layer"),
    rs_float("oreamount", "Ore amount", 0.0, 3.0, "x", "Layer"),
    rs_float(
        "layermodifierchance",
        "Layer modifier chance",
        0.0,
        100.0,
        "%",
        "Layer",
    ),
    rs_float(
        "timebetweenearthquakes",
        "Time between earthquakes",
        0.1,
        5.0,
        "x",
        "Layer",
    ),
    rs_whole(
        "temperatureoffset",
        "Temperature offset",
        -80.0,
        50.0,
        "°C",
        "Layer",
    ),
    rs_bool("allowlayerendmovement", "Allow layer-end movement", "Layer"),
    RunSettingDef {
        id: "startingsupplies",
        label: "Starting supplies",
        kind: RunSettingKind::Dropdown(&["None", "Light", "Full"]),
        postfix: "",
        section: "Player",
    },
    rs_float("xpgain", "XP gain", 0.0, 5.0, "x", "Player"),
    rs_float("metabolismrate", "Metabolism rate", 0.1, 2.0, "x", "Player"),
    rs_float("healingrate", "Healing rate", 0.1, 3.0, "x", "Player"),
    rs_float("fracturepain", "Fracture pain", 0.0, 1.5, "x", "Player"),
    rs_float("bleedrate", "Bleed rate", 0.2, 2.0, "x", "Player"),
    rs_float("infectionspeed", "Infection speed", 0.1, 3.0, "x", "Player"),
    rs_float(
        "infectionchance",
        "Infection chance",
        0.0,
        3.0,
        "x",
        "Player",
    ),
    rs_float(
        "fibrillationrate",
        "Fibrillation rate",
        0.1,
        3.0,
        "x",
        "Player",
    ),
    rs_float(
        "moodnormalizationrate",
        "Mood normalisation rate",
        0.0,
        3.0,
        "x",
        "Player",
    ),
    rs_float("bonuslimbarmor", "Bonus limb armor", 0.0, 4.0, "", "Player"),
    rs_float(
        "staminaregen",
        "Stamina regeneration",
        0.5,
        2.0,
        "x",
        "Player",
    ),
    rs_float("attackdamage", "Attack damage", 0.2, 3.0, "x", "Player"),
    rs_float(
        "minigamehandshake",
        "Minigame hand shake",
        0.0,
        2.0,
        "x",
        "Player",
    ),
    rs_float(
        "sleepcyclespeed",
        "Sleep cycle speed",
        0.1,
        2.0,
        "x",
        "Player",
    ),
    rs_float("encumbrancecap", "Encumbrance cap", 0.5, 3.0, "x", "Player"),
    rs_float(
        "insulationmultiplier",
        "Insulation multiplier",
        0.5,
        2.0,
        "x",
        "Player",
    ),
    rs_bool("strokes", "Strokes", "Player"),
    rs_bool("braindamagefx", "Brain damage effects", "Player"),
    rs_bool("forcesleep", "Force sleep", "Player"),
    rs_bool("lowmoodevents", "Low mood events", "Player"),
    rs_bool("liquidpushing", "Liquid pushing", "Player"),
    rs_bool("disfigurement", "Disfigurement", "Player"),
    rs_bool("crushdepth", "Crush depth", "Player"),
    rs_bool("nosleeprestrictions", "No sleep restrictions", "Player"),
    rs_bool("infinitelaststand", "Infinite last stand", "Player"),
    rs_float("traderchance", "Trader chance", 0.0, 100.0, "%", "Trader"),
    rs_float(
        "traderitemamount",
        "Trader item amount",
        0.2,
        3.0,
        "x",
        "Trader",
    ),
    rs_float(
        "traderrepoffset",
        "Trader reputation offset",
        -100.0,
        100.0,
        "",
        "Trader",
    ),
    rs_float("enemystrength", "Enemy strength", 0.2, 2.0, "x", "Other"),
    rs_float("itemdecayrate", "Item decay rate", 0.0, 3.0, "x", "Other"),
    rs_float(
        "lockpickprecision",
        "Lockpick precision",
        0.5,
        3.0,
        "x",
        "Other",
    ),
    rs_bool("permanentaddiction", "Permanent addiction", "Challenges"),
    rs_bool("mindwiped", "Mind-wiped", "Challenges"),
    rs_bool("zeroskills", "Zero skills", "Challenges"),
    rs_bool("blind", "Blind", "Challenges"),
    rs_bool("amputee", "Amputee", "Challenges"),
    rs_bool("immortalsnail", "Immortal snail", "Challenges"),
];

/// Looks up a run setting definition by id.
pub fn run_setting(id: &str) -> Option<&'static RunSettingDef> {
    RUN_SETTINGS.iter().find(|d| d.id == id)
}
