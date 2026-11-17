use crate::game_data::runtime::{RuntimeSlice, metadata};

/// An ending discovered from the game's preference/achievement writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EndingDef {
    pub pref_key: &'static str,
    pub achievement: &'static str,
    pub name: &'static str,
    pub status_key: &'static str,
    pub status: &'static str,
    pub how: &'static str,
}

pub const ENDINGS: RuntimeSlice<EndingDef> = RuntimeSlice(|| &metadata().endings);
