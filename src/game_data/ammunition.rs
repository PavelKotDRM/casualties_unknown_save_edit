//! Ammunition and weapon defaults from the selected game's prefabs.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct AmmoDef {
    pub ammo_type: i64,
    pub magazine: bool,
    pub insertable: bool,
    pub defaults: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GunDef {
    pub ammo_type: i64,
    pub magazine_fed: bool,
    pub defaults: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChamberValues {
    pub round: i64,
    pub casing: i64,
    pub empty: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AmmunitionData {
    pub items: BTreeMap<String, AmmoDef>,
    pub guns: BTreeMap<String, GunDef>,
    pub chamber: ChamberValues,
}
