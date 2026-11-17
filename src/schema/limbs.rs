use super::FieldSpec;
use crate::game_data::runtime::{RuntimeSlice, metadata};

pub const LIMB_FIELDS: RuntimeSlice<FieldSpec> = RuntimeSlice(|| &metadata().limb_fields);

pub fn limb_field(key: &str) -> Option<&'static FieldSpec> {
    LIMB_FIELDS.snapshot().iter().find(|field| field.key == key)
}

/// A serialized limb slot, read from the player's body prefab.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimbDef {
    pub object: &'static str,
    pub name: &'static str,
}

pub const LIMBS: RuntimeSlice<LimbDef> = RuntimeSlice(|| &metadata().limbs);
