//! Catalogue of identifiers read from the selected game installation.
//!
//! The data is used only for autocompletion and grouping in the editor; any
//! unknown id found in a save is still displayed and preserved as-is.
//! Display names are loaded at runtime from the game's own language files
//! (see [`crate::locale`]). Without game data both catalogues are empty.

use crate::game_data::runtime::{RuntimeSlice, metadata};

/// Static description of an item prefab known to the game.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemDef {
    /// Prefab id as stored in the save (`items[].id`).
    pub id: &'static str,
    /// In-game category (`medical`, `food`, `container`, ...).
    pub category: &'static str,
    /// Wear slot id for wearables (empty if the item cannot be worn).
    pub wear_slot: &'static str,
    /// Liquid capacity for liquid containers (0 if not a liquid container).
    pub capacity: f32,
}

pub const ITEMS: RuntimeSlice<ItemDef> = RuntimeSlice(|| &metadata().items);
pub const LIQUIDS: RuntimeSlice<&str> = RuntimeSlice(|| &metadata().liquids);

/// Looks up an item definition by id.
pub fn item(id: &str) -> Option<&'static ItemDef> {
    ITEMS.snapshot().iter().find(|d| d.id == id)
}
