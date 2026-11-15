//! Structural operations on the save document.
//!
//! These functions encapsulate edits that must keep several parts of the
//! save consistent, e.g. the parallel `items` / `itemComponents` arrays, or
//! skill levels and their experience boundaries. They are UI-independent and
//! unit tested.

use serde_json::{Map, Value, json};

use crate::json::{get_f64, get_i64, set_bool, set_f64, set_i64};
use crate::schema;

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

fn items(data: &Value) -> &[Value] {
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
pub fn new_item(id: &str, location: &Location) -> Value {
    let mut item = json!({
        "id": id,
        "condition": 1.0,
        "slot": -1,
        "wearSlot": null,
        "favourited": false,
        "label": "",
        "swatch": 0
    });
    location.apply(&mut item);
    item
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

/// Heals every limb: full skin/muscle health, no pain, bleeding, infection,
/// fractures, dislocations or shrapnel. Dismembered limbs are restored only
/// when `regrow` is set.
pub fn heal_limbs(data: &mut Value, regrow: bool) {
    let Some(limbs) = data.get_mut("limbs").and_then(Value::as_array_mut) else {
        return;
    };
    for limb in limbs.iter_mut().filter_map(Value::as_object_mut) {
        heal_limb(limb, regrow);
    }
}

/// Heals a single limb object (see [`heal_limbs`]).
pub fn heal_limb(limb: &mut Map<String, Value>, regrow: bool) {
    if !regrow && limb.get("dismembered").and_then(Value::as_bool) == Some(true) {
        return;
    }
    for key in ["skinHealth", "muscleHealth"] {
        set_f64(limb, key, 100.0);
    }
    for key in ["pain", "bleedAmount", "infectionAmount", "furBloodAmount"] {
        set_f64(limb, key, 0.0);
    }
    for key in [
        "dislocated",
        "broken",
        "infected",
        "splinted",
        "blockedBleeding",
    ] {
        set_bool(limb, key, false);
    }
    if regrow {
        set_bool(limb, "dismembered", false);
    }
    set_i64(limb, "shrapnel", 0);
}

/// Restores vital signs and removes acute conditions of the body.
pub fn restore_vitals(data: &mut Value) {
    let Some(body) = data.get_mut("body").and_then(Value::as_object_mut) else {
        return;
    };
    let values: &[(&str, f64)] = &[
        ("bloodVolume", 100.0),
        ("bloodOxygen", 100.0),
        ("heartRate", 70.0),
        ("bloodPressure", 120.0),
        ("respiratoryRate", 100.0),
        ("fibrillationProgress", 0.0),
        ("consciousness", 100.0),
        ("brainHealth", 100.0),
        ("shock", 0.0),
        ("painShock", 0.0),
        ("traumaAmount", 0.0),
        ("internalBleeding", 0.0),
        ("hemothorax", 0.0),
        ("strokeAmount", 0.0),
        ("septicShock", 0.0),
        ("sicknessAmount", 0.0),
        ("radiationSickness", 0.0),
        ("hearingLoss", 0.0),
        ("temperature", 37.0),
        ("venomCurrent", 0.0),
    ];
    for &(key, value) in values {
        if body.contains_key(key) {
            set_f64(body, key, value);
        }
    }
    for key in ["fibrillationForced", "hasPulmonaryEmbolism"] {
        if body.contains_key(key) {
            set_bool(body, key, false);
        }
    }
}

/// Satisfies food, water, stamina and sleep needs.
pub fn fill_needs(data: &mut Value) {
    let Some(body) = data.get_mut("body").and_then(Value::as_object_mut) else {
        return;
    };
    for (key, value) in [
        ("hunger", 100.0),
        ("thirst", 100.0),
        ("stamina", 100.0),
        ("energy", 100.0),
        ("dirtyness", 0.0),
        ("wetness", 0.0),
    ] {
        if body.contains_key(key) {
            set_f64(body, key, value);
        }
    }
}

/// Sets a skill level and updates its experience boundaries the same way the
/// game does (`Skills.UpdateExpBoundaries`). The relative progress towards the
/// next level is kept. The derived `<KEY>From10` value is updated as well.
pub fn set_skill_level(skills: &mut Map<String, Value>, key: &str, level: i64) {
    let level = level.clamp(0, 99);
    let old_min = get_f64(skills, &format!("min{key}")).unwrap_or(0.0);
    let old_max = get_f64(skills, &format!("max{key}")).unwrap_or(0.0);
    let exp = get_f64(skills, &format!("exp{key}")).unwrap_or(old_min);
    let progress = if old_max > old_min {
        ((exp - old_min) / (old_max - old_min)).clamp(0.0, 0.999)
    } else {
        0.0
    };
    let min = schema::experience_for_level(level as i32);
    let max = schema::experience_for_level(level as i32 + 1);
    set_i64(skills, key, level);
    set_i64(skills, &format!("min{key}"), min);
    set_i64(skills, &format!("max{key}"), max);
    set_f64(
        skills,
        &format!("exp{key}"),
        min as f64 + progress * (max - min) as f64,
    );
    if skills.contains_key(&format!("{key}From10")) {
        set_f64(skills, &format!("{key}From10"), level as f64 - 10.0);
    }
}

/// Returns `(level, progress 0..1)` for a skill.
pub fn skill_progress(skills: &Map<String, Value>, key: &str) -> (i64, f64) {
    let level = get_i64(skills, key).unwrap_or(0);
    let min = get_f64(skills, &format!("min{key}")).unwrap_or(0.0);
    let max = get_f64(skills, &format!("max{key}")).unwrap_or(0.0);
    let exp = get_f64(skills, &format!("exp{key}")).unwrap_or(min);
    let p = if max > min {
        (exp - min) / (max - min)
    } else {
        0.0
    };
    (level, p.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Value {
        json!({
            "body": {"skills": {"STR": 13, "expSTR": 1158.5, "minSTR": 1062, "maxSTR": 1255, "STRFrom10": 3.0}},
            "limbs": [{"skinHealth": 10.0, "broken": true, "dismembered": false, "shrapnel": 2},
                      {"skinHealth": 0.0, "dismembered": true}],
            "items": [
                {"id": "medkit", "slot": 5, "wearSlot": null},
                {"id": "bandage", "slot": 5, "wearSlot": null},
                {"id": "bigpack", "slot": -1, "wearSlot": "back"},
                {"id": "cereal", "slot": -1, "wearSlot": "back"}
            ],
            "itemComponents": [{"m": 1}, {}, {}, {"c": 1}],
            "biome": 2
        })
    }

    fn ids(d: &Value) -> Vec<&str> {
        d["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].as_str().unwrap())
            .collect()
    }

    #[test]
    fn groups_follow_holder_then_contents() {
        let g = group_items(&doc());
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].location, Location::Slot(5));
        assert_eq!(g[1].indices, vec![2, 3]);
    }

    #[test]
    fn components_stay_aligned_with_items() {
        let mut d = doc();
        let i = add_item(&mut d, new_item("lantern", &Location::Slot(3)), json!({})).unwrap();
        assert_eq!(i, 2, "new slot groups go after existing slot items");
        assert_eq!(
            ids(&d),
            ["medkit", "bandage", "lantern", "bigpack", "cereal"]
        );
        assert_eq!(d["itemComponents"][4], json!({"c": 1}));

        remove_item(&mut d, 0).unwrap();
        assert_eq!(d["itemComponents"].as_array().unwrap().len(), 4);
        assert_eq!(d["itemComponents"][3], json!({"c": 1}));

        let j = move_to_location(&mut d, 3, &Location::Slot(5)).unwrap();
        assert_eq!(ids(&d), ["bandage", "cereal", "lantern", "bigpack"]);
        assert_eq!(d["itemComponents"][j], json!({"c": 1}));
        assert_eq!(d["items"][j]["wearSlot"], Value::Null);

        let keys: Vec<_> = d.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["body", "limbs", "items", "itemComponents", "biome"]);
    }

    #[test]
    fn move_within_group_swaps_components_too() {
        let mut d = doc();
        assert_eq!(move_within_group(&mut d, 3, true), Some(2));
        assert_eq!(ids(&d)[2], "cereal");
        assert_eq!(d["itemComponents"][2], json!({"c": 1}));
        assert_eq!(
            move_within_group(&mut d, 2, true),
            None,
            "cannot leave the group"
        );
    }

    #[test]
    fn heal_respects_dismemberment_unless_regrowing() {
        let mut d = doc();
        heal_limbs(&mut d, false);
        assert_eq!(d["limbs"][0]["skinHealth"], json!(100.0));
        assert_eq!(d["limbs"][0]["broken"], json!(false));
        assert_eq!(d["limbs"][0]["shrapnel"], json!(0));
        assert_eq!(d["limbs"][1]["skinHealth"], json!(0.0));
        heal_limbs(&mut d, true);
        assert_eq!(d["limbs"][1]["dismembered"], json!(false));
    }

    #[test]
    fn skill_level_change_updates_boundaries_and_keeps_progress() {
        let mut d = doc();
        let skills = d["body"]["skills"].as_object_mut().unwrap();
        set_skill_level(skills, "STR", 14);
        assert_eq!(skills["STR"], json!(14));
        assert_eq!(skills["minSTR"], json!(1255));
        assert_eq!(skills["maxSTR"], json!(1484));
        assert_eq!(skills["STRFrom10"], json!(4.0));
        let (_, p) = skill_progress(skills, "STR");
        assert!((p - 0.5).abs() < 1e-3);
    }
}
