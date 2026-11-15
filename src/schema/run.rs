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

/// Value type used by the game for one world setting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PresetValue {
    /// Toggle.
    Bool(bool),
    /// Slider value.
    Float(f32),
    /// Dropdown option index.
    Index(i64),
}

impl PresetValue {
    fn to_json(self) -> serde_json::Value {
        match self {
            Self::Bool(value) => serde_json::Value::Bool(value),
            Self::Float(value) => serde_json::json!(value),
            Self::Index(value) => serde_json::Value::from(value),
        }
    }
}

/// One built-in world setup offered by the game.
#[derive(Debug, Clone, Copy)]
pub struct RunPreset {
    /// Stable preset id.
    pub id: &'static str,
    /// English fallback name.
    pub name: &'static str,
    /// Values that differ from the normal preset.
    pub overrides: &'static [(&'static str, PresetValue)],
}

const NORMAL_SETTINGS: &[(&str, PresetValue)] = &[
    ("unchipped", PresetValue::Bool(false)),
    ("endless", PresetValue::Bool(false)),
    ("baselootdensity", PresetValue::Float(1.0)),
    ("lootmultiplier", PresetValue::Float(1.0)),
    ("basetrapdensity", PresetValue::Float(1.0)),
    ("trapincrease", PresetValue::Float(0.0)),
    ("ambientlight", PresetValue::Index(0)),
    ("timelimit", PresetValue::Float(60.0)),
    ("oreamount", PresetValue::Float(1.0)),
    ("layermodifierchance", PresetValue::Float(40.0)),
    ("timebetweenearthquakes", PresetValue::Float(1.0)),
    ("temperatureoffset", PresetValue::Float(0.0)),
    ("allowlayerendmovement", PresetValue::Bool(false)),
    ("startingsupplies", PresetValue::Index(1)),
    ("xpgain", PresetValue::Float(1.0)),
    ("metabolismrate", PresetValue::Float(1.0)),
    ("healingrate", PresetValue::Float(1.0)),
    ("fracturepain", PresetValue::Float(1.0)),
    ("bleedrate", PresetValue::Float(1.0)),
    ("infectionspeed", PresetValue::Float(1.0)),
    ("infectionchance", PresetValue::Float(1.0)),
    ("fibrillationrate", PresetValue::Float(1.0)),
    ("moodnormalizationrate", PresetValue::Float(1.0)),
    ("bonuslimbarmor", PresetValue::Float(0.0)),
    ("staminaregen", PresetValue::Float(1.0)),
    ("attackdamage", PresetValue::Float(1.0)),
    ("minigamehandshake", PresetValue::Float(1.0)),
    ("sleepcyclespeed", PresetValue::Float(1.0)),
    ("encumbrancecap", PresetValue::Float(1.0)),
    ("insulationmultiplier", PresetValue::Float(1.0)),
    ("strokes", PresetValue::Bool(true)),
    ("braindamagefx", PresetValue::Bool(true)),
    ("forcesleep", PresetValue::Bool(true)),
    ("lowmoodevents", PresetValue::Bool(true)),
    ("liquidpushing", PresetValue::Bool(true)),
    ("disfigurement", PresetValue::Bool(true)),
    ("crushdepth", PresetValue::Bool(true)),
    ("nosleeprestrictions", PresetValue::Bool(false)),
    ("infinitelaststand", PresetValue::Bool(false)),
    ("traderchance", PresetValue::Float(33.0)),
    ("traderitemamount", PresetValue::Float(1.0)),
    ("traderrepoffset", PresetValue::Float(0.0)),
    ("enemystrength", PresetValue::Float(1.0)),
    ("itemdecayrate", PresetValue::Float(1.0)),
    ("lockpickprecision", PresetValue::Float(1.0)),
    ("permanentaddiction", PresetValue::Bool(false)),
    ("mindwiped", PresetValue::Bool(false)),
    ("zeroskills", PresetValue::Bool(false)),
    ("blind", PresetValue::Bool(false)),
    ("amputee", PresetValue::Bool(false)),
    ("immortalsnail", PresetValue::Bool(false)),
];

/// Built-in world presets read from the game's `RunSettings` initializer.
pub const RUN_PRESETS: &[RunPreset] = &[
    RunPreset {
        id: "normal",
        name: "Normal",
        overrides: &[],
    },
    RunPreset {
        id: "relaxed",
        name: "Relaxed",
        overrides: &[
            ("baselootdensity", PresetValue::Float(1.15)),
            ("basetrapdensity", PresetValue::Float(0.8)),
            ("ambientlight", PresetValue::Index(1)),
            ("timelimit", PresetValue::Float(120.0)),
            ("startingsupplies", PresetValue::Index(2)),
            ("enemystrength", PresetValue::Float(0.75)),
            ("xpgain", PresetValue::Float(1.15)),
            ("metabolismrate", PresetValue::Float(0.7)),
            ("healingrate", PresetValue::Float(1.2)),
            ("fracturepain", PresetValue::Float(0.67)),
            ("bleedrate", PresetValue::Float(0.75)),
            ("infectionspeed", PresetValue::Float(0.7)),
            ("infectionchance", PresetValue::Float(0.75)),
            ("fibrillationrate", PresetValue::Float(0.8)),
            ("moodnormalizationrate", PresetValue::Float(1.25)),
            ("staminaregen", PresetValue::Float(1.1)),
            ("minigamehandshake", PresetValue::Float(0.66)),
            ("insulationmultiplier", PresetValue::Float(1.15)),
            ("forcesleep", PresetValue::Bool(false)),
            ("crushdepth", PresetValue::Bool(false)),
            ("traderrepoffset", PresetValue::Float(15.0)),
            ("layermodifierchance", PresetValue::Float(20.0)),
            ("itemdecayrate", PresetValue::Float(0.75)),
            ("lockpickprecision", PresetValue::Float(1.5)),
            ("timebetweenearthquakes", PresetValue::Float(1.5)),
        ],
    },
    RunPreset {
        id: "paradise",
        name: "Paradise",
        overrides: &[
            ("baselootdensity", PresetValue::Float(1.5)),
            ("basetrapdensity", PresetValue::Float(0.35)),
            ("ambientlight", PresetValue::Index(2)),
            ("timelimit", PresetValue::Float(180.0)),
            ("startingsupplies", PresetValue::Index(2)),
            ("enemystrength", PresetValue::Float(0.5)),
            ("allowlayerendmovement", PresetValue::Bool(true)),
            ("xpgain", PresetValue::Float(1.25)),
            ("metabolismrate", PresetValue::Float(0.7)),
            ("healingrate", PresetValue::Float(1.5)),
            ("fracturepain", PresetValue::Float(0.4)),
            ("bleedrate", PresetValue::Float(0.7)),
            ("infectionspeed", PresetValue::Float(0.5)),
            ("infectionchance", PresetValue::Float(0.75)),
            ("fibrillationrate", PresetValue::Float(0.6)),
            ("moodnormalizationrate", PresetValue::Float(1.25)),
            ("staminaregen", PresetValue::Float(1.2)),
            ("minigamehandshake", PresetValue::Float(0.2)),
            ("insulationmultiplier", PresetValue::Float(1.3)),
            ("forcesleep", PresetValue::Bool(false)),
            ("liquidpushing", PresetValue::Bool(false)),
            ("crushdepth", PresetValue::Bool(false)),
            ("infinitelaststand", PresetValue::Bool(true)),
            ("attackdamage", PresetValue::Float(1.5)),
            ("encumbrancecap", PresetValue::Float(1.5)),
            ("traderrepoffset", PresetValue::Float(25.0)),
            ("traderchance", PresetValue::Float(50.0)),
            ("traderitemamount", PresetValue::Float(1.25)),
            ("itemdecayrate", PresetValue::Float(0.5)),
            ("layermodifierchance", PresetValue::Float(10.0)),
            ("lockpickprecision", PresetValue::Float(2.5)),
            ("bonuslimbarmor", PresetValue::Float(0.25)),
            ("timebetweenearthquakes", PresetValue::Float(2.0)),
        ],
    },
    RunPreset {
        id: "desolate",
        name: "Desolate",
        overrides: &[
            ("baselootdensity", PresetValue::Float(0.4)),
            ("basetrapdensity", PresetValue::Float(1.5)),
            ("startingsupplies", PresetValue::Index(0)),
            ("fracturepain", PresetValue::Float(1.25)),
            ("moodnormalizationrate", PresetValue::Float(0.5)),
            ("traderrepoffset", PresetValue::Float(-10.0)),
            ("traderchance", PresetValue::Float(10.0)),
            ("traderitemamount", PresetValue::Float(3.0)),
            ("minigamehandshake", PresetValue::Float(2.0)),
            ("layermodifierchance", PresetValue::Float(100.0)),
            ("temperatureoffset", PresetValue::Float(-15.0)),
            ("oreamount", PresetValue::Float(0.5)),
            ("timebetweenearthquakes", PresetValue::Float(0.8)),
        ],
    },
    RunPreset {
        id: "unchipped",
        name: "Unchipped",
        overrides: &[
            ("unchipped", PresetValue::Bool(true)),
            ("timelimit", PresetValue::Float(300.0)),
            ("startingsupplies", PresetValue::Index(0)),
        ],
    },
    RunPreset {
        id: "endless",
        name: "Endless",
        overrides: &[
            ("endless", PresetValue::Bool(true)),
            ("lootmultiplier", PresetValue::Float(0.95)),
            ("trapincrease", PresetValue::Float(0.06)),
        ],
    },
];

/// Build the serialized setting list for one built-in game preset.
pub fn run_settings_for_preset(id: &str) -> Option<Vec<serde_json::Value>> {
    let preset = RUN_PRESETS.iter().find(|preset| preset.id == id)?;
    Some(
        RUN_SETTINGS
            .iter()
            .map(|setting| {
                let value = preset
                    .overrides
                    .iter()
                    .find_map(|(id, value)| (*id == setting.id).then_some(*value))
                    .or_else(|| {
                        NORMAL_SETTINGS
                            .iter()
                            .find_map(|(id, value)| (*id == setting.id).then_some(*value))
                    })
                    .expect("every run setting has a game default")
                    .to_json();
                serde_json::json!({"Item1": setting.id, "Item2": value})
            })
            .collect(),
    )
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
