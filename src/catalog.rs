//! Static catalogue of known game identifiers (item ids, liquid ids).
//!
//! The data is used only for autocompletion and grouping in the editor; any
//! unknown id found in a save is still displayed and preserved as-is.
//! Display names are loaded at runtime from the game's own language files
//! (see [`crate::locale`]), so only identifiers live here.

/// Static description of an item prefab known to the game.
#[derive(Debug, Clone, Copy)]
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

mod items;
mod liquids;

pub use items::ITEMS;
pub use liquids::LIQUIDS;

/// Looks up an item definition by id.
pub fn item(id: &str) -> Option<&'static ItemDef> {
    ITEMS.iter().find(|d| d.id == id)
}
