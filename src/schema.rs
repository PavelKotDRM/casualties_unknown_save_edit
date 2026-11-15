//! Knowledge about the meaning of save fields.
//!
//! Each known field gets a human-readable label, a description, a sensible
//! range and a hint about which values are "good". The UI uses this to render
//! gauges and colour-code values similar to the in-game health screen.
//! Ranges mirror the clamps used by the game code where they exist; values
//! outside the range can still be entered through the numeric editors.

/// Which values of a stat are healthy; used for colour coding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Good {
    /// Higher is better (health, blood volume, ...).
    High,
    /// Lower is better (pain, shock, bleeding, ...).
    Low,
    /// Values close to the given target are best (heart rate, temperature).
    Near(f32),
    /// No judgement; displayed neutrally.
    Neutral,
}

/// How a field is edited and visualised.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// Bounded number, shown as a gauge with a slider.
    Gauge,
    /// Free number (timers, physics constants), shown as a drag value.
    Number,
    /// Integer counter.
    Int,
    /// Boolean flag.
    Flag,
}

/// Metadata of a single field.
#[derive(Debug, Clone, Copy)]
pub struct FieldSpec {
    /// JSON key.
    pub key: &'static str,
    /// Display label.
    pub label: &'static str,
    /// Tooltip text.
    pub help: &'static str,
    /// Editing / display kind.
    pub kind: Kind,
    /// Lower bound of the gauge/slider.
    pub min: f32,
    /// Upper bound of the gauge/slider.
    pub max: f32,
    /// Unit suffix shown after the value.
    pub unit: &'static str,
    /// Which values are healthy.
    pub good: Good,
}

const fn gauge(
    key: &'static str,
    label: &'static str,
    min: f32,
    max: f32,
    unit: &'static str,
    good: Good,
    help: &'static str,
) -> FieldSpec {
    FieldSpec {
        key,
        label,
        help,
        kind: Kind::Gauge,
        min,
        max,
        unit,
        good,
    }
}

const fn num(key: &'static str, label: &'static str, help: &'static str) -> FieldSpec {
    FieldSpec {
        key,
        label,
        help,
        kind: Kind::Number,
        min: f32::NEG_INFINITY,
        max: f32::INFINITY,
        unit: "",
        good: Good::Neutral,
    }
}

const fn int(key: &'static str, label: &'static str, help: &'static str) -> FieldSpec {
    FieldSpec {
        key,
        label,
        help,
        kind: Kind::Int,
        min: f32::NEG_INFINITY,
        max: f32::INFINITY,
        unit: "",
        good: Good::Neutral,
    }
}

const fn flag(key: &'static str, label: &'static str, help: &'static str) -> FieldSpec {
    FieldSpec {
        key,
        label,
        help,
        kind: Kind::Flag,
        min: 0.0,
        max: 1.0,
        unit: "",
        good: Good::Neutral,
    }
}

/// A titled group of body fields.
#[derive(Debug, Clone, Copy)]
pub struct FieldGroup {
    /// Group title.
    pub title: &'static str,
    /// Icon shown before the title.
    pub icon: &'static str,
    /// Fields of the group in display order.
    pub fields: &'static [FieldSpec],
}

use Good::{High, Low, Near, Neutral};

/// Groups of fields of the `body` object (everything except `skills`).
pub const BODY_GROUPS: &[FieldGroup] = &[
    FieldGroup {
        title: "Circulation & breathing",
        icon: "❤",
        fields: &[
            gauge(
                "bloodVolume",
                "Blood volume",
                0.0,
                100.0,
                "%",
                High,
                "Amount of blood. Low values cause hypovolemia and death.",
            ),
            gauge(
                "bloodOxygen",
                "Blood oxygen",
                0.0,
                100.0,
                "%",
                High,
                "Oxygen saturation. Below 50% you pass out.",
            ),
            gauge(
                "heartRate",
                "Heart rate",
                0.0,
                250.0,
                " bpm",
                Near(70.0),
                "Beats per minute.",
            ),
            gauge(
                "bloodPressure",
                "Blood pressure",
                0.0,
                250.0,
                " mmHg",
                Near(120.0),
                "Systolic blood pressure.",
            ),
            gauge(
                "respiratoryRate",
                "Respiratory rate",
                0.0,
                100.0,
                "%",
                High,
                "Breathing efficiency.",
            ),
            gauge(
                "fibrillationProgress",
                "Fibrillation",
                0.0,
                100.0,
                "%",
                Low,
                "Arrhythmia progress. Above 60% the heart stops pumping effectively.",
            ),
            flag(
                "fibrillationForced",
                "Forced fibrillation",
                "Fibrillation is forced by an effect.",
            ),
            gauge(
                "internalBleeding",
                "Internal bleeding",
                0.0,
                100.0,
                "",
                Low,
                "Internal blood loss.",
            ),
            gauge(
                "hemothorax",
                "Hemothorax",
                0.0,
                100.0,
                "",
                Low,
                "Blood in the chest cavity; treat with a chest drain.",
            ),
            flag(
                "hasPulmonaryEmbolism",
                "Pulmonary embolism",
                "Blood clot in the lungs.",
            ),
            gauge(
                "strokeAmount",
                "Stroke",
                0.0,
                100.0,
                "",
                Low,
                "Stroke severity.",
            ),
            gauge(
                "bloodViscosity",
                "Blood viscosity",
                -100.0,
                100.0,
                "",
                Near(0.0),
                "Thick (+) or thin (-) blood; affects clotting and strokes.",
            ),
            gauge(
                "bloodVesselSize",
                "Blood vessel size",
                0.85,
                1.15,
                "x",
                Near(1.0),
                "Vessel dilation multiplier.",
            ),
            gauge(
                "heartRatePressureOffset",
                "Heart-rate pressure offset",
                -30.0,
                80.0,
                "",
                Near(0.0),
                "Pressure offset caused by the heart rate.",
            ),
            num(
                "bloodPressureChangeFromMedicine",
                "Pressure change (medicine)",
                "Blood pressure change caused by drugs.",
            ),
            gauge(
                "adrenaline",
                "Adrenaline",
                0.0,
                100.0,
                "",
                Neutral,
                "Adrenaline level.",
            ),
            gauge(
                "curAdrenaline",
                "Current adrenaline",
                0.0,
                100.0,
                "",
                Neutral,
                "Currently active adrenaline.",
            ),
        ],
    },
    FieldGroup {
        title: "Needs",
        icon: "🍖",
        fields: &[
            gauge(
                "hunger",
                "Satiety",
                -50.0,
                125.0,
                "",
                High,
                "Food level. 100 = full, above 100 = overfed, below 0 = starving.",
            ),
            gauge(
                "thirst",
                "Hydration",
                -50.0,
                250.0,
                "",
                Near(100.0),
                "Water level. 100 = hydrated, above 125 slows you down.",
            ),
            gauge(
                "stamina",
                "Stamina",
                0.0,
                100.0,
                "",
                High,
                "Physical stamina.",
            ),
            gauge(
                "energy",
                "Energy",
                0.0,
                100.0,
                "",
                High,
                "Sleepiness: 0 = exhausted.",
            ),
            gauge(
                "caffeinated",
                "Caffeine",
                0.0,
                200.0,
                "",
                Neutral,
                "Caffeine in the system.",
            ),
            gauge(
                "weightOffset",
                "Weight offset",
                -80.0,
                100.0,
                " kg",
                Near(0.0),
                "Body weight change from the base weight.",
            ),
            gauge(
                "dirtyness",
                "Dirtiness",
                0.0,
                100.0,
                "%",
                Low,
                "How dirty you are; raises infection risk.",
            ),
            gauge(
                "wetness",
                "Wetness",
                0.0,
                100.0,
                "%",
                Low,
                "How wet the fur is.",
            ),
            gauge(
                "snowAmount",
                "Snow",
                0.0,
                100.0,
                "",
                Low,
                "Snow stuck on the body.",
            ),
            num("badSleepAmount", "Bad sleep", "Accumulated bad sleep."),
            num(
                "goodSleepTime",
                "Good sleep time",
                "Accumulated good sleep.",
            ),
        ],
    },
    FieldGroup {
        title: "Mind",
        icon: "🧠",
        fields: &[
            gauge("happiness", "Mood", -100.0, 100.0, "", High, "Base mood."),
            gauge(
                "consciousness",
                "Consciousness",
                0.0,
                100.0,
                "%",
                High,
                "Level of consciousness.",
            ),
            gauge(
                "brainHealth",
                "Brain health",
                0.0,
                100.0,
                "%",
                High,
                "Brain damage accumulates from hypoxia and trauma.",
            ),
            gauge("shock", "Shock", 0.0, 100.0, "", Low, "Circulatory shock."),
            gauge(
                "painShock",
                "Pain shock",
                0.0,
                100.0,
                "",
                Low,
                "Shock caused by pain.",
            ),
            gauge(
                "traumaAmount",
                "Trauma",
                0.0,
                100.0,
                "",
                Low,
                "Psychological trauma.",
            ),
            gauge(
                "desensitizedMult",
                "Desensitisation",
                0.0,
                1.0,
                "x",
                Low,
                "Lower = less affected by gore (corpses seen).",
            ),
            int("corpsesSeen", "Corpses seen", "Number of corpses seen."),
            gauge(
                "brainGrowSickness",
                "Brain-grow sickness",
                0.0,
                100.0,
                "",
                Low,
                "Side effect of brain-grow serum.",
            ),
            flag(
                "usedNeuralBooster",
                "Used neural booster",
                "Neural booster was used.",
            ),
        ],
    },
    FieldGroup {
        title: "Sickness & immunity",
        icon: "🦠",
        fields: &[
            gauge(
                "temperature",
                "Body temperature",
                27.0,
                42.0,
                " °C",
                Near(37.5),
                "Below 27 °C or above 41.5 °C is lethal.",
            ),
            gauge(
                "sicknessAmount",
                "Nausea / sickness",
                0.0,
                100.0,
                "",
                Low,
                "General sickness.",
            ),
            gauge(
                "septicShock",
                "Septic shock",
                0.0,
                100.0,
                "",
                Low,
                "Sepsis. Above 82.5 you lose consciousness.",
            ),
            gauge(
                "radiationSickness",
                "Radiation sickness",
                0.0,
                100.0,
                "",
                Low,
                "Radiation poisoning.",
            ),
            gauge(
                "immunity",
                "Immunity",
                0.0,
                200.0,
                "",
                High,
                "Resistance to infections.",
            ),
            num(
                "antibioticImmunityTime",
                "Antibiotic time",
                "Remaining antibiotic protection time.",
            ),
            gauge(
                "hearingLoss",
                "Hearing loss",
                0.0,
                100.0,
                "",
                Low,
                "Temporary hearing loss.",
            ),
            num("venomTotal", "Venom (total)", "Total venom received."),
            num("venomCurrent", "Venom (current)", "Venom currently active."),
            num("venomFake", "Venom (fake)", "Venom-like effect."),
        ],
    },
    FieldGroup {
        title: "Body",
        icon: "🐾",
        fields: &[
            gauge(
                "clawHealth",
                "Claw health",
                0.0,
                100.0,
                "%",
                High,
                "Condition of the claws.",
            ),
            num(
                "clawRegrowTime",
                "Claw regrow time",
                "Time until claws regrow.",
            ),
            flag("disfigured", "Disfigured", "Face is disfigured."),
            flag("eyeGone", "Lost an eye", "One eye is missing."),
            flag(
                "bothEyesGone",
                "Lost both eyes",
                "Both eyes are missing (blind).",
            ),
            flag(
                "triedRollingLastStand",
                "Tried last stand",
                "A last stand roll was attempted.",
            ),
            flag(
                "succesfullyRolledLastStand",
                "Last stand succeeded",
                "The last stand roll succeeded.",
            ),
            num("lastStandTime", "Last stand time", "Last stand timer."),
        ],
    },
    FieldGroup {
        title: "Movement",
        icon: "🏃",
        fields: &[
            num("maxSpeed", "Max speed", "Maximum running speed."),
            num("jumpSpeed", "Jump speed", "Jump strength."),
            num("moveForce", "Move force", "Acceleration force."),
            num(
                "slowdownAmount",
                "Slowdown amount",
                "Movement slowdown factor.",
            ),
            num(
                "temporarySlowdown",
                "Temporary slowdown",
                "Temporary movement penalty.",
            ),
        ],
    },
];

/// Looks up a body field spec by key.
pub fn body_field(key: &str) -> Option<&'static FieldSpec> {
    BODY_GROUPS
        .iter()
        .flat_map(|g| g.fields.iter())
        .find(|f| f.key == key)
}

/// Fields of a limb object (except `prostheticInfo`).
pub const LIMB_FIELDS: &[FieldSpec] = &[
    gauge(
        "skinHealth",
        "Skin health",
        0.0,
        100.0,
        "%",
        High,
        "Skin integrity. Damaged skin bleeds and gets infected.",
    ),
    gauge(
        "muscleHealth",
        "Muscle health",
        0.0,
        100.0,
        "%",
        High,
        "Muscle integrity. Affects strength of the limb.",
    ),
    gauge("pain", "Pain", 0.0, 100.0, "", Low, "Pain in this limb."),
    gauge(
        "bleedAmount",
        "Bleeding",
        0.0,
        100.0,
        "",
        Low,
        "Bleeding intensity.",
    ),
    gauge(
        "infectionAmount",
        "Infection",
        0.0,
        100.0,
        "%",
        Low,
        "Infection progress.",
    ),
    flag("infected", "Infected", "The wound is infected."),
    flag("broken", "Broken", "The bone is fractured."),
    flag("dislocated", "Dislocated", "The joint is dislocated."),
    flag("splinted", "Splinted", "A splint is applied."),
    flag(
        "blockedBleeding",
        "Tourniquet",
        "Bleeding is blocked (tourniquet).",
    ),
    flag("dismembered", "Dismembered", "The limb is missing."),
    int(
        "shrapnel",
        "Shrapnel",
        "Number of shrapnel pieces stuck in the limb.",
    ),
    gauge(
        "furBloodAmount",
        "Blood on fur",
        0.0,
        100.0,
        "",
        Low,
        "Cosmetic blood on the fur.",
    ),
    num(
        "disinfectionTime",
        "Disinfection time",
        "Remaining disinfection effect.",
    ),
    num(
        "bandageSlowAmount",
        "Bandage slow amount",
        "Bleeding reduction from bandages.",
    ),
    num("skinHealAmount", "Skin heal amount", "Skin healing bonus."),
    num(
        "dislocationTimer",
        "Dislocation timer",
        "Timer of the dislocation healing.",
    ),
    num(
        "boneHealTimer",
        "Bone heal timer",
        "Timer of the fracture healing.",
    ),
];

/// Looks up a limb field spec by key.
pub fn limb_field(key: &str) -> Option<&'static FieldSpec> {
    LIMB_FIELDS.iter().find(|f| f.key == key)
}

/// Static description of a limb slot of the `limbs` array.
#[derive(Debug, Clone, Copy)]
pub struct LimbDef {
    /// Unity game-object name; also the locale key of the limb name.
    pub object: &'static str,
    /// English fallback name.
    pub name: &'static str,
}

/// The 15 limbs in the order the game stores them.
///
/// `F` = front (right side), `B` = back (left side).
pub const LIMBS: [LimbDef; 15] = [
    LimbDef {
        object: "Head",
        name: "Head",
    },
    LimbDef {
        object: "UpTorso",
        name: "Thorax",
    },
    LimbDef {
        object: "DownTorso",
        name: "Abdomen",
    },
    LimbDef {
        object: "UpArmF",
        name: "Upper right arm",
    },
    LimbDef {
        object: "DownArmF",
        name: "Right forearm",
    },
    LimbDef {
        object: "HandF",
        name: "Right hand",
    },
    LimbDef {
        object: "UpArmB",
        name: "Upper left arm",
    },
    LimbDef {
        object: "DownArmB",
        name: "Left forearm",
    },
    LimbDef {
        object: "HandB",
        name: "Left hand",
    },
    LimbDef {
        object: "ThighF",
        name: "Right thigh",
    },
    LimbDef {
        object: "CrusF",
        name: "Right crus",
    },
    LimbDef {
        object: "FootF",
        name: "Right foot",
    },
    LimbDef {
        object: "ThighB",
        name: "Left thigh",
    },
    LimbDef {
        object: "CrusB",
        name: "Left crus",
    },
    LimbDef {
        object: "FootB",
        name: "Left foot",
    },
];

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experience_matches_values_written_by_the_game() {
        // Taken from a real save: STR 13 -> [1062, 1255), RES 14 -> [1255, 1484), INT 12 -> [897, 1062).
        assert_eq!(experience_for_level(12), 897);
        assert_eq!(experience_for_level(13), 1062);
        assert_eq!(experience_for_level(14), 1255);
        assert_eq!(experience_for_level(15), 1484);
        assert_eq!(experience_for_level(0), 0);
    }
}
