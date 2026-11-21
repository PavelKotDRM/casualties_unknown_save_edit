//! Inventory locations and aligned item/component edits.

use serde_json::{Value, json};

/// Where an item is located in the inventory.
///
/// Within one location the first item is the one held / worn; following
/// items are stored *inside* it (it must be a container). This mirrors how
/// `SaveSystem.TryLoadGame` restores items.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Location {
    /// Inventory (hand / hotbar) slot index (`slot >= 0`).
    Slot(i64),
    /// Wear slot id (`slot == -1`, `wearSlot == id`).
    Worn(String),
}

impl Location {
    /// Location of a single item object.
    pub fn of(item: &Value) -> Location {
        let slot = item.get("slot").and_then(Value::as_i64).unwrap_or(-1);
        if slot >= 0 {
            Location::Slot(slot)
        } else {
            Location::Worn(
                item.get("wearSlot")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            )
        }
    }

    /// Writes this location into an item object.
    pub fn apply(&self, item: &mut Value) {
        let Some(obj) = item.as_object_mut() else {
            return;
        };
        match self {
            Location::Slot(s) => {
                obj.insert("slot".into(), json!(s));
                obj.insert("wearSlot".into(), Value::Null);
            }
            Location::Worn(w) => {
                obj.insert("slot".into(), json!(-1));
                obj.insert("wearSlot".into(), json!(w));
            }
        }
    }
}

/// Items sharing one [`Location`].
#[derive(Debug, Clone, PartialEq)]
pub struct ItemGroup {
    /// Location of the group.
    pub location: Location,
    /// Indices into `items`; the first is the holder / worn item.
    pub indices: Vec<usize>,
}

pub(super) fn items(data: &Value) -> &[Value] {
    data.get("items")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Groups items by location, in order of first appearance.
pub fn group_items(data: &Value) -> Vec<ItemGroup> {
    let mut groups: Vec<ItemGroup> = Vec::new();
    for (i, item) in items(data).iter().enumerate() {
        let loc = Location::of(item);
        match groups.iter_mut().find(|g| g.location == loc) {
            Some(g) => g.indices.push(i),
            None => groups.push(ItemGroup {
                location: loc,
                indices: vec![i],
            }),
        }
    }
    groups
}

/// Makes sure `items` and `itemComponents` exist and have the same length,
/// and returns mutable references to both arrays.
fn arrays_mut(data: &mut Value) -> Option<(&mut Vec<Value>, &mut Vec<Value>)> {
    let obj = data.as_object_mut()?;
    if !obj.get("itemComponents").is_some_and(Value::is_array) {
        obj.insert("itemComponents".into(), json!([]));
    }
    let (mut it, mut co) = (None, None);
    for (key, value) in obj.iter_mut() {
        match (key.as_str(), value) {
            ("items", Value::Array(a)) => it = Some(a),
            ("itemComponents", Value::Array(a)) => co = Some(a),
            _ => {}
        }
    }
    let (it, co) = (it?, co?);
    co.resize(it.len(), json!({}));
    Some((it, co))
}

/// Builds a new item object with default values.
pub fn new_item(id: &str, location: &Location) -> Result<Value, String> {
    if id.trim().is_empty() {
        return Err("An item id is required".into());
    }
    let defaults = &crate::game_data::runtime::metadata().item_defaults;
    let mut object = serde_json::Map::new();
    for key in ["condition", "favourited", "label", "swatch"] {
        let value = defaults
            .get(key)
            .ok_or_else(|| format!("Game default for item {key} is unavailable"))?;
        object.insert(key.into(), value.clone());
    }
    object.insert("id".into(), id.into());
    let components = new_item_components(id)?;
    if let Some(stack) = components
        .get(crate::game_data::component_key("WaterContainerItem"))
        .and_then(|component| component.get("stack"))
        .and_then(Value::as_array)
    {
        let capacity = crate::catalog::item(id)
            .map(|item| item.capacity)
            .filter(|capacity| capacity.is_finite() && *capacity > 0.0)
            .ok_or_else(|| format!("Liquid capacity for {id} is unavailable"))?;
        let total = stack
            .iter()
            .try_fold(0.0, |total, entry| {
                entry
                    .get("amount")
                    .and_then(Value::as_f64)
                    .filter(|amount| amount.is_finite() && *amount >= 0.0)
                    .map(|amount| total + amount)
            })
            .filter(|total| total.is_finite())
            .ok_or_else(|| format!("Invalid default liquid contents for {id}"))?;
        object.insert("condition".into(), json!(total / f64::from(capacity)));
    }
    if let Some(battery) = components.get(crate::game_data::component_key("BatteryItem"))
        && battery
            .get("batteryType")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
    {
        object.insert("condition".into(), json!(0.0));
    }
    let mut item = Value::Object(object);
    location.apply(&mut item);
    Ok(item)
}

/// Saved contents from the item's resource prefab and game initialization.
pub fn new_item_components(id: &str) -> Result<Value, String> {
    let metadata = crate::game_data::runtime::metadata();
    if let Some(error) = &metadata.item_components_error {
        return Err(format!("Item component defaults unavailable: {error}"));
    }
    if let Some(error) = &metadata.ammunition_error {
        return Err(format!("Item ammunition defaults unavailable: {error}"));
    }
    Ok(metadata
        .item_components
        .get(id)
        .cloned()
        .unwrap_or_else(|| json!({})))
}

/// Index at which a new item for `location` should be inserted: after the
/// last item of that location, or — for a new location — after all inventory
/// slot items (for slots) or at the end (for wear slots).
fn insertion_index(data: &Value, location: &Location) -> usize {
    let all = items(data);
    if let Some(last) = all.iter().rposition(|it| Location::of(it) == *location) {
        return last + 1;
    }
    match location {
        Location::Slot(_) => all
            .iter()
            .rposition(|it| matches!(Location::of(it), Location::Slot(_)))
            .map_or(0, |i| i + 1),
        Location::Worn(_) => all.len(),
    }
}

/// Adds an item (with optional components) and returns its index.
pub fn add_item(data: &mut Value, item: Value, components: Value) -> Option<usize> {
    let location = Location::of(&item);
    let index = insertion_index(data, &location);
    let (it, co) = arrays_mut(data)?;
    it.insert(index, item);
    co.insert(index, components);
    Some(index)
}

/// Removes the item at `index` together with its components.
pub fn remove_item(data: &mut Value, index: usize) -> Option<Value> {
    let (it, co) = arrays_mut(data)?;
    if index >= it.len() {
        return None;
    }
    co.remove(index);
    Some(it.remove(index))
}

/// Duplicates the item at `index` (including components) right after it.
pub fn duplicate_item(data: &mut Value, index: usize) -> Option<usize> {
    let (it, co) = arrays_mut(data)?;
    let item = it.get(index)?.clone();
    let comp = co.get(index).cloned().unwrap_or_else(|| json!({}));
    it.insert(index + 1, item);
    co.insert(index + 1, comp);
    Some(index + 1)
}

/// Swaps the item at `index` with the previous (`up`) or next item of the
/// same location. Returns the new index.
pub fn move_within_group(data: &mut Value, index: usize, up: bool) -> Option<usize> {
    let group = group_items(data)
        .into_iter()
        .find(|g| g.indices.contains(&index))?;
    let pos = group.indices.iter().position(|&i| i == index)?;
    let other = if up {
        *group.indices.get(pos.checked_sub(1)?)?
    } else {
        *group.indices.get(pos + 1)?
    };
    let (it, co) = arrays_mut(data)?;
    it.swap(index, other);
    co.swap(index, other);
    Some(other)
}

/// Moves an item to another location (appending it to that location).
/// Returns the new index.
pub fn move_to_location(data: &mut Value, index: usize, location: &Location) -> Option<usize> {
    let mut item = items(data).get(index)?.clone();
    if Location::of(&item) == *location {
        return Some(index);
    }
    let comp = data
        .get("itemComponents")
        .and_then(|c| c.get(index))
        .cloned()
        .unwrap_or_else(|| json!({}));
    remove_item(data, index)?;
    location.apply(&mut item);
    add_item(data, item, comp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_liquid_and_battery_defaults_do_not_create_full_items() {
        let _metadata = crate::game_data::runtime::scoped_for_test(
            crate::game_data::metadata::Metadata {
                item_defaults: serde_json::from_value(json!({
                    "condition": 1.0, "favourited": false, "label": "", "swatch": 0
                })).unwrap(),
                items: vec![crate::catalog::ItemDef {
                    id: "bottle", category: "tool", group: "drinks", wear_slot: "", capacity: 800.0,
                }],
                item_components: [
                    ("bottle".into(), json!({crate::game_data::component_key("WaterContainerItem"): {"stack": []}})),
                    ("device".into(), json!({crate::game_data::component_key("BatteryItem"): {"batteryType": null, "maxCharge": 0.0}})),
                ].into(),
                ..Default::default()
            }
        );
        for id in ["bottle", "device"] {
            assert_eq!(new_item(id, &Location::Slot(0)).unwrap()["condition"], 0.0);
        }
    }
}
