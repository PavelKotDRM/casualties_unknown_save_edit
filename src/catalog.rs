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
    /// Editor group id from [`ITEM_GROUPS`], derived from the game data.
    pub group: &'static str,
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

/// Editor grouping of items. The game's own categories mix clothing,
/// weapons and devices (`utility`) and put most crafted items in `custom`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemGroup {
    pub id: &'static str,
    /// English label, translated through the interface language.
    pub label: &'static str,
    pub icon: &'static str,
}

/// All groups in display order.
pub const ITEM_GROUPS: &[ItemGroup] = &[
    ItemGroup {
        id: "weapons",
        label: "Weapons & ammo",
        icon: "🔫",
    },
    ItemGroup {
        id: "tools",
        label: "Tools & melee weapons",
        icon: "🔧",
    },
    ItemGroup {
        id: "clothing",
        label: "Clothing & armour",
        icon: "👕",
    },
    ItemGroup {
        id: "containers",
        label: "Bags & containers",
        icon: "🎒",
    },
    ItemGroup {
        id: "medical",
        label: "Medical",
        icon: "✚",
    },
    ItemGroup {
        id: "drugs",
        label: "Drugs",
        icon: "💊",
    },
    ItemGroup {
        id: "food",
        label: "Food",
        icon: "🍞",
    },
    ItemGroup {
        id: "drinks",
        label: "Drinks",
        icon: "💧",
    },
    ItemGroup {
        id: "devices",
        label: "Devices & gear",
        icon: "🔦",
    },
    ItemGroup {
        id: "materials",
        label: "Materials",
        icon: "🔩",
    },
    ItemGroup {
        id: "unobtainable",
        label: "Unobtainable",
        icon: "⛔",
    },
    ItemGroup {
        id: "other",
        label: "Other",
        icon: "📦",
    },
];

/// The group with this id, or "Other".
pub fn group(id: &str) -> &'static ItemGroup {
    ITEM_GROUPS
        .iter()
        .find(|group| group.id == id)
        .unwrap_or(&ITEM_GROUPS[ITEM_GROUPS.len() - 1])
}

/// Position of a group in [`ITEM_GROUPS`], for sorting.
pub fn group_order(id: &str) -> usize {
    ITEM_GROUPS
        .iter()
        .position(|group| group.id == id)
        .unwrap_or(ITEM_GROUPS.len())
}

/// Facts about an item used to choose its group.
pub struct GroupFacts<'a> {
    pub category: &'a str,
    pub tags: &'a std::collections::HashSet<String>,
    pub worn: bool,
    /// Holds items and gives neither armour nor insulation (a bag, not clothing).
    pub bag: bool,
    /// A gun, round or magazine in the game's ammunition data.
    pub ammunition: bool,
    /// Using the item feeds or waters the player directly.
    pub edible: bool,
    /// Using the item drinks its liquid contents.
    pub drinkable: bool,
}

/// Chooses the editor group of an item; earlier rules win.
pub fn classify(facts: &GroupFacts<'_>) -> &'static str {
    let tag = |name: &str| facts.tags.contains(name);
    match facts.category {
        _ if facts.ammunition || tag("gun") || tag("bullet") => "weapons",
        "container" => "containers",
        _ if facts.bag => "containers",
        _ if facts.worn => "clothing",
        _ if tag("tool") => "tools",
        "tool" => "tools",
        "drug" => "drugs",
        "medical" => "medical",
        _ if tag("medicine") => "medical",
        "water" => "drinks",
        "food" => "food",
        "unobtainable" => "unobtainable",
        _ if facts.edible => "food",
        _ if facts.drinkable => "drinks",
        "utility" => "devices",
        "trash" => "materials",
        _ if tag("material") => "materials",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn classify_item(
        category: &str,
        tags: &[&str],
        worn: bool,
        bag: bool,
        edible: bool,
    ) -> &'static str {
        let tags: HashSet<String> = tags.iter().map(|tag| (*tag).to_owned()).collect();
        classify(&GroupFacts {
            category,
            tags: &tags,
            worn,
            bag,
            ammunition: false,
            edible,
            drinkable: false,
        })
    }

    #[test]
    fn utility_and_custom_items_are_split_into_useful_groups() {
        assert_eq!(
            classify_item("utility", &["gun"], false, false, false),
            "weapons"
        );
        assert_eq!(
            classify_item("utility", &[], true, false, false),
            "clothing"
        );
        assert_eq!(
            classify_item("custom", &[], true, true, false),
            "containers"
        );
        assert_eq!(
            classify_item("custom", &["tool"], false, false, false),
            "tools"
        );
        assert_eq!(
            classify_item("utility", &[], false, false, false),
            "devices"
        );
        assert_eq!(classify_item("custom", &[], false, false, true), "food");
        assert_eq!(
            classify_item("unobtainable", &[], false, false, true),
            "unobtainable"
        );
        assert_eq!(
            classify_item("custom", &["material"], false, false, false),
            "materials"
        );
        assert_eq!(classify_item("custom", &[], false, false, false), "other");
    }
}
