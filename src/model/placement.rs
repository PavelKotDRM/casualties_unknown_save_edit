//! Inventory destination rules shared by adding, moving, and repairing items.

use std::collections::HashMap;

use serde_json::Value;

use crate::game_data::ItemStats;

use super::{self as model, Location};

/// Requirements an item places on the location where it is the holder.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HolderRules<'a> {
    /// Wear slot the item can be worn in.
    pub wear_slot: Option<&'a str>,
    /// The game only lets the item occupy a hand slot.
    pub hands_only: bool,
    /// Clothing that can only be worn or stored, never held in a body slot.
    pub not_holdable: bool,
    /// Limb object the item is worn on; it cannot be worn while that limb is missing.
    pub wear_limb: Option<&'a str>,
}

/// Validated shape of an available destination.
#[derive(Debug, Clone, PartialEq)]
pub enum Destination {
    /// An empty slot where the item would become the holder.
    Empty,
    /// An existing container with enough room for the item.
    Container {
        holder_id: String,
        remaining: f32,
        capacity: f32,
    },
}

/// Why an item cannot be placed at a destination.
#[derive(Debug, Clone, PartialEq)]
pub enum Rejection {
    /// The selected item is missing from the save or lacks an id.
    ItemDataUnavailable,
    /// Required game metadata for the chosen container is missing.
    ContainerDataUnavailable(String),
    /// A known item occupies the location but is not a container.
    OccupiedByNonContainer(String),
    /// The item cannot be worn because its corresponding limb is missing.
    MissingWearLimb(usize),
    /// Clothing cannot be held directly in a body slot.
    ClothingNotHoldable,
    /// The item can only be held in hands.
    HandsOnly,
    /// The item has no compatible wear slot.
    CannotWear,
    /// A filled container cannot be nested inside another container.
    FilledContainer,
    /// The target container has no usable item-weight limit.
    WeightLimitUnavailable(String),
    /// Container acceptance restrictions could not be checked.
    RestrictionsUnavailable(String),
    /// Item type metadata is unavailable for a restricted container.
    ItemTypeUnavailable(String),
    /// The target container rejects this item's type.
    ItemTypeNotAccepted(String),
    /// Item weight cannot be verified.
    ItemWeightUnavailable(String),
    /// The item exceeds the container's maximum weight per item.
    ItemTooHeavy(String),
    /// Current container capacity cannot be verified.
    CapacityUnavailable(String),
    /// The container is already over capacity.
    OverCapacity(String),
    /// The container has no remaining capacity.
    Full(String),
    /// The item is heavier than the available space.
    NotEnoughCapacity(String),
}

impl Rejection {
    /// Whether the rejection is caused by unavailable or incomplete metadata.
    pub fn is_metadata_warning(&self) -> bool {
        matches!(
            self,
            Self::ItemDataUnavailable
                | Self::ContainerDataUnavailable(_)
                | Self::WeightLimitUnavailable(_)
                | Self::RestrictionsUnavailable(_)
                | Self::ItemTypeUnavailable(_)
                | Self::ItemWeightUnavailable(_)
                | Self::CapacityUnavailable(_)
        )
    }
}

/// Wear slot available for `id`, preferring synchronized game metadata.
pub fn item_wear_slot<'a>(id: &str, stats: &'a HashMap<String, ItemStats>) -> Option<&'a str> {
    if let Some(item) = stats.get(id) {
        return item.wearable.then_some(item.wear_slot.as_deref()).flatten();
    }
    crate::catalog::item(id)
        .filter(|item| !item.wear_slot.is_empty())
        .map(|item| item.wear_slot)
}

/// Requirements an item places on the location where it is the holder.
pub fn holder_rules<'a>(id: &str, stats: &'a HashMap<String, ItemStats>) -> HolderRules<'a> {
    HolderRules {
        wear_slot: item_wear_slot(id, stats),
        hands_only: stats.get(id).is_some_and(|item| item.only_hold_in_hands),
        not_holdable: match stats.get(id) {
            Some(item) => item.wearable && !item.wearable_can_be_held,
            None => item_wear_slot(id, stats).is_some(),
        },
        wear_limb: stats
            .get(id)
            .and_then(|item| item.desired_wear_limb.as_deref()),
    }
}

/// Index of the limb a wearable goes on when that limb is dismembered.
pub fn missing_wear_limb(data: &Value, holder: HolderRules<'_>) -> Option<usize> {
    let index = crate::schema::LIMBS
        .iter()
        .position(|limb| Some(limb.object) == holder.wear_limb)?;
    data.get("limbs")
        .and_then(|limbs| limbs.get(index))
        .and_then(|limb| limb.get("dismembered"))
        .and_then(Value::as_bool)
        .filter(|missing| *missing)
        .map(|_| index)
}

/// Where a new or reset item should go by default.
pub fn default_location(holder: HolderRules<'_>) -> Location {
    match holder.wear_slot {
        Some(slot) if holder.not_holdable => Location::Worn(slot.to_owned()),
        _ => Location::Slot(0),
    }
}

/// Whether `slot` is one of the character's hand slots.
pub fn is_hand_slot(slot: i64) -> bool {
    matches!(slot, 0 | 1)
}

/// Locations offered in choosers, with only compatible wear slots.
pub fn location_choices(
    data: &Value,
    holder: HolderRules<'_>,
    current: &Location,
    preserve_current: bool,
) -> Vec<Location> {
    let mut out: Vec<Location> = model::group_items(data)
        .into_iter()
        .map(|group| group.location)
        .filter(|location| holder_location_allowed(data, location, holder))
        .collect();
    for slot in 0..crate::schema::HOLD_SLOTS.len() as i64 {
        let location = Location::Slot(slot);
        if !out.contains(&location) {
            out.push(location);
        }
    }
    if let Some(slot) = holder.wear_slot {
        let location = Location::Worn(slot.to_owned());
        if !out.contains(&location) {
            out.push(location);
        }
    }
    if preserve_current && !out.contains(current) {
        out.push(current.clone());
    }
    out
}

/// Whether a location may be targeted as the item's holder.
pub fn holder_location_allowed(data: &Value, location: &Location, holder: HolderRules<'_>) -> bool {
    let occupied = || {
        model::group_items(data)
            .iter()
            .any(|group| &group.location == location)
    };
    match location {
        Location::Slot(slot) => {
            (!holder.not_holdable && (!holder.hands_only || is_hand_slot(*slot))) || occupied()
        }
        Location::Worn(slot) => {
            (holder.wear_slot == Some(slot.as_str()) && missing_wear_limb(data, holder).is_none())
                || (!slot.is_empty() && occupied())
        }
    }
}

/// Validates adding or moving `item_id` into `location`.
///
/// `item_index` is the item's current save index when moving; it enables
/// accurate liquid/condition weights and prevents nesting a filled container.
pub fn check_destination(
    data: &Value,
    stats: &HashMap<String, ItemStats>,
    item_id: &str,
    location: &Location,
    item_index: Option<usize>,
) -> Result<Destination, Rejection> {
    let groups = model::group_items(data);
    let Some(group) = groups.iter().find(|group| &group.location == location) else {
        let holder = holder_rules(item_id, stats);
        if !holder_location_allowed(data, location, holder) {
            return Err(not_holdable(data, location, holder));
        }
        return Ok(Destination::Empty);
    };
    let Some(&root_index) = group.indices.first() else {
        return Err(Rejection::ContainerDataUnavailable(String::new()));
    };
    let Some(root_id) = items(data)
        .get(root_index)
        .and_then(|item| item.get("id"))
        .and_then(Value::as_str)
    else {
        return Err(Rejection::ContainerDataUnavailable(String::new()));
    };
    let moving_filled_container = item_index.is_some_and(|index| {
        groups
            .iter()
            .any(|group| group.indices.len() > 1 && group.indices.first() == Some(&index))
    });
    if moving_filled_container {
        return Err(Rejection::FilledContainer);
    }

    let Some(container_stats) = stats.get(root_id) else {
        return if crate::catalog::item(root_id).is_some_and(|item| item.category != "container") {
            Err(Rejection::OccupiedByNonContainer(root_id.to_owned()))
        } else {
            Err(Rejection::ContainerDataUnavailable(root_id.to_owned()))
        };
    };
    let Some(capacity) = container_stats.container_capacity else {
        return if crate::catalog::item(root_id).is_some_and(|item| item.category != "container") {
            Err(Rejection::OccupiedByNonContainer(root_id.to_owned()))
        } else {
            Err(Rejection::ContainerDataUnavailable(root_id.to_owned()))
        };
    };
    let Some(max_item_weight) = container_stats
        .container_max_item_weight
        .filter(|weight| weight.is_finite() && *weight >= 0.0)
    else {
        return Err(Rejection::WeightLimitUnavailable(root_id.to_owned()));
    };
    let Some(restrictions) = container_stats.container_tag_restriction.as_ref() else {
        return Err(Rejection::RestrictionsUnavailable(root_id.to_owned()));
    };
    if !restrictions.is_empty() {
        let Some(item_stats) = stats.get(item_id) else {
            return Err(Rejection::ItemTypeUnavailable(root_id.to_owned()));
        };
        if container_stats.accepts_item(item_stats) == Some(false) {
            return Err(Rejection::ItemTypeNotAccepted(root_id.to_owned()));
        }
    }
    let item_weight = if let Some(index) = item_index {
        let weights = model::item_weights(data, index, stats);
        weights.game_complete.then_some(weights.game).flatten()
    } else {
        stats.get(item_id).and_then(|item_stats| {
            if item_stats.liquid_capacity.is_none() {
                return Some(item_stats.total_weight(1.0));
            }
            let item = model::new_item(item_id, location).ok()?;
            let components = model::new_item_components(item_id).ok()?;
            let candidate = serde_json::json!({"items": [item], "itemComponents": [components]});
            let weights = model::item_weights(&candidate, 0, stats);
            weights.game_complete.then_some(weights.game).flatten()
        })
    }
    .filter(|weight| weight.is_finite() && *weight >= 0.0)
    .ok_or_else(|| Rejection::ItemWeightUnavailable(root_id.to_owned()))?;
    if item_weight > max_item_weight + f32::EPSILON {
        return Err(Rejection::ItemTooHeavy(root_id.to_owned()));
    }
    let space = model::container_space(data, root_index, stats)
        .filter(|space| space.complete)
        .ok_or_else(|| Rejection::CapacityUnavailable(root_id.to_owned()))?;
    if space.over_capacity > 0.0 {
        return Err(Rejection::OverCapacity(root_id.to_owned()));
    }
    if space.remaining <= f32::EPSILON {
        return Err(Rejection::Full(root_id.to_owned()));
    }
    if item_weight > space.remaining + f32::EPSILON {
        return Err(Rejection::NotEnoughCapacity(root_id.to_owned()));
    }
    Ok(Destination::Container {
        holder_id: root_id.to_owned(),
        remaining: space.remaining,
        capacity,
    })
}

fn items(data: &Value) -> &[Value] {
    data.get("items")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn not_holdable(data: &Value, location: &Location, holder: HolderRules<'_>) -> Rejection {
    if let Location::Worn(_) = location
        && let Some(limb) = missing_wear_limb(data, holder)
    {
        return Rejection::MissingWearLimb(limb);
    }
    match location {
        Location::Slot(_) if holder.not_holdable => Rejection::ClothingNotHoldable,
        Location::Slot(_) => Rejection::HandsOnly,
        Location::Worn(_) => Rejection::CannotWear,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use serde_json::json;

    use super::*;

    fn stats(tags: &[&str]) -> ItemStats {
        ItemStats {
            category: "medical".to_owned(),
            weight: 1.0,
            trade_value: 0.0,
            untradeable: false,
            scale_weight_with_condition: false,
            wearable: false,
            wear_slot: None,
            armor: None,
            insulation: None,
            item_tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            container_capacity: None,
            container_max_item_weight: None,
            container_encumbrance: None,
            container_tag_restriction: None,
            liquid_capacity: None,
            only_hold_in_hands: false,
            wearable_can_be_held: false,
            desired_wear_limb: None,
            destroy_at_zero_condition: false,
        }
    }

    #[test]
    fn hands_only_item_rejects_an_empty_non_hand_slot() {
        let stats = HashMap::from([(
            "tool".to_owned(),
            ItemStats {
                only_hold_in_hands: true,
                ..stats(&[])
            },
        )]);

        assert_eq!(
            check_destination(
                &json!({"items": []}),
                &stats,
                "tool",
                &Location::Slot(4),
                None
            ),
            Err(Rejection::HandsOnly)
        );
    }

    #[test]
    fn container_restrictions_and_weight_limits_are_checked_before_placement() {
        let mut container = stats(&[]);
        container.container_capacity = Some(8.0);
        container.container_max_item_weight = Some(2.0);
        container.container_encumbrance = Some(0.5);
        container.container_tag_restriction = Some(HashSet::from(["medical".to_owned()]));
        let save = json!({"items": [{"id": "case", "slot": 0}]});

        let restricted = HashMap::from([
            ("case".to_owned(), container.clone()),
            ("food".to_owned(), stats(&["food"])),
        ]);
        assert_eq!(
            check_destination(&save, &restricted, "food", &Location::Slot(0), None),
            Err(Rejection::ItemTypeNotAccepted("case".to_owned()))
        );

        container.container_tag_restriction = Some(HashSet::new());
        let too_heavy = HashMap::from([
            ("case".to_owned(), container),
            (
                "heavy".to_owned(),
                ItemStats {
                    weight: 3.0,
                    ..stats(&[])
                },
            ),
        ]);
        assert_eq!(
            check_destination(&save, &too_heavy, "heavy", &Location::Slot(0), None),
            Err(Rejection::ItemTooHeavy("case".to_owned()))
        );
    }

    #[test]
    fn compatible_item_gets_a_validated_container_destination() {
        let mut container = stats(&[]);
        container.container_capacity = Some(8.0);
        container.container_max_item_weight = Some(2.0);
        container.container_encumbrance = Some(0.5);
        container.container_tag_restriction = Some(HashSet::new());
        let stats = HashMap::from([
            ("case".to_owned(), container),
            ("bandage".to_owned(), stats(&[])),
        ]);

        assert_eq!(
            check_destination(
                &json!({"items": [{"id": "case", "slot": 0}]}),
                &stats,
                "bandage",
                &Location::Slot(0),
                None,
            ),
            Ok(Destination::Container {
                holder_id: "case".to_owned(),
                remaining: 8.0,
                capacity: 8.0,
            })
        );
    }
}
