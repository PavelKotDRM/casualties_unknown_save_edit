use super::*;
use Good::{High, Low};

/// Fields of a limb object (except `prostheticInfo`).
// FIXME: Limb field IDs, labels, descriptions, gauge ranges/units, and healthy-value targets are hard-coded.
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
// FIXME: The 15 limb object IDs, display names, and save-array order are hard-coded.
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
