//! Structural operations on the save document.
//!
//! These functions encapsulate edits that must keep several parts of the
//! save consistent, e.g. the parallel `items` / `itemComponents` arrays, or
//! skill levels and their experience boundaries. They are UI-independent and
//! unit tested.

// FIXME: ~850 lines covering four unrelated domains: inventory weights/capacity (`item_weights`,
// `physical_load`, `container_space`, `carry_load`), inventory structure (`Location`, `ItemGroup`,
// `group_items`, `new_item`/`add_item`/`remove_item`/`duplicate_item`/`move_*`), body healing
// (`heal_limbs`, `heal_limb`, `restore_vitals`, `fill_needs`) and skills (`set_skill_level`,
// `setup_character_skills`, `skill_progress`). Split into `model/{inventory.rs, load.rs,
// health.rs, skills.rs}` with `model.rs` re-exporting, and move the per-function tests along
// with them. `carry_load` (~80 lines) should be broken into named steps (base capacity, weight
// offset, sickness, hunger/thirst, skills) so the hard-coded formula is readable and testable.

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use crate::game_data::ItemStats;
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

/// Computed carrying load and capacity in game units (`u`), with incomplete metadata flagged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CarryLoad {
    pub weight: f32,
    pub capacity: f32,
    pub complete: bool,
}

/// Game-unit and estimated physical weights for one item. Physical weight is not a game stat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemWeights {
    pub game: Option<f32>,
    pub game_complete: bool,
    pub physical: Option<f32>,
    pub liquid: Option<f32>,
}

/// Estimated total physical weight, without the game's container modifiers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicalLoad {
    pub weight: f32,
    pub complete: bool,
}

/// Remaining container capacity in game units (`u`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContainerSpace {
    pub capacity: f32,
    pub used: f32,
    pub remaining: f32,
    pub over_capacity: f32,
    pub complete: bool,
}

/// Compute game and estimated physical weights from the item and its saved components.
pub fn item_weights(data: &Value, index: usize, stats: &HashMap<String, ItemStats>) -> ItemWeights {
    let unknown = ItemWeights {
        game: None,
        game_complete: false,
        physical: None,
        liquid: None,
    };
    let Some(item) = items(data).get(index) else {
        return unknown;
    };
    let Some(item_stats) = item
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| stats.get(id))
    else {
        return unknown;
    };
    let Some(capacity) = item_stats.liquid_capacity else {
        let weight = item_stats.total_weight(item_condition(item));
        return ItemWeights {
            game: Some(weight),
            game_complete: true,
            physical: Some(weight),
            liquid: None,
        };
    };
    if !capacity.is_finite() || capacity <= 0.0 {
        return unknown;
    }

    let volume = data
        .get("itemComponents")
        .and_then(Value::as_array)
        .and_then(|components| components.get(index))
        .and_then(Value::as_object)
        .and_then(|components| {
            let mut matching = components.iter().filter(|(key, _)| {
                key.split(',')
                    .next()
                    .is_some_and(|name| name.trim() == "WaterContainerItem")
            });
            let (_, component) = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            let stack = component.get("stack")?.as_array()?;
            stack.iter().try_fold(0.0_f64, |total, entry| {
                let amount = entry.get("amount")?.as_f64()?;
                (amount.is_finite() && amount >= 0.0).then_some(total + amount)
            })
        })
        .filter(|volume| volume.is_finite() && *volume <= f32::MAX as f64)
        .map(|volume| volume as f32);
    let Some(volume) = volume else {
        return ItemWeights {
            game: Some(item_stats.total_weight(item_condition(item))),
            ..unknown
        };
    };
    let liquid_weight = volume / 1000.0;
    let game = item_stats.total_weight(volume / capacity);
    let physical = item_stats.total_weight(0.0) + liquid_weight;
    ItemWeights {
        game: game.is_finite().then_some(game),
        game_complete: game.is_finite(),
        physical: physical.is_finite().then_some(physical),
        liquid: liquid_weight.is_finite().then_some(liquid_weight),
    }
}

/// Sum physical item estimates independently of carrying-capacity data.
pub fn physical_load(data: &Value, stats: &HashMap<String, ItemStats>) -> PhysicalLoad {
    let mut load = PhysicalLoad {
        weight: 0.0,
        complete: data.get("items").is_some_and(Value::is_array),
    };
    for index in 0..items(data).len() {
        match item_weights(data, index, stats).physical {
            Some(weight) if (load.weight + weight).is_finite() => load.weight += weight,
            _ => load.complete = false,
        }
    }
    load
}

/// Compute remaining storage capacity from the full, unmodified weight of the contents.
pub fn container_space(
    data: &Value,
    index: usize,
    stats: &HashMap<String, ItemStats>,
) -> Option<ContainerSpace> {
    let item = items(data).get(index)?;
    let id = item.get("id").and_then(Value::as_str)?;
    let capacity = stats.get(id)?.container_capacity?;
    if !capacity.is_finite() || capacity < 0.0 {
        return None;
    }

    let group = group_items(data)
        .into_iter()
        .find(|group| group.indices.first() == Some(&index))?;
    let mut used = 0.0;
    let mut complete = true;
    for &child_index in group.indices.iter().skip(1) {
        let weight = item_weights(data, child_index, stats);
        complete &= weight.game_complete;
        if let Some(value) = weight.game.filter(|value| value.is_finite()) {
            if (used + value).is_finite() {
                used += value;
            } else {
                complete = false;
            }
        } else {
            complete = false;
        }
    }

    let remaining = (capacity - used).max(0.0);
    let over_capacity = (used - capacity).max(0.0);
    Some(ContainerSpace {
        capacity,
        used,
        remaining,
        over_capacity,
        complete,
    })
}

// FIXME: The missing run-setting fallback hard-codes the game's normal `encumbrancecap` value (1.0).
const DEFAULT_ENCUMBRANCE_CAP: f64 = 1.0;

/// Computes the game's encumbrance value from equipped items, container
/// contents, body state and run settings.
pub fn carry_load(data: &Value, stats: &HashMap<String, ItemStats>) -> Option<CarryLoad> {
    let body = data.get("body")?;
    let skills = body.get("skills")?;
    let field = |key: &str| body.get(key).and_then(Value::as_f64).map(|v| v as f32);
    let weight_offset = field("weightOffset")?;
    let sickness = field("sicknessAmount")?;
    let hunger = field("hunger")?;
    let thirst = field("thirst")?;
    let strength = skills.get("STRFrom10").and_then(Value::as_f64)? as f32;
    let resistance = skills.get("RESFrom10").and_then(Value::as_f64)? as f32;
    let run_cap = data
        .get("runSettings")
        .and_then(Value::as_array)
        .and_then(|settings| {
            settings.iter().find_map(|setting| {
                (setting.get("Item1").and_then(Value::as_str) == Some("encumbrancecap"))
                    .then(|| setting.get("Item2").and_then(Value::as_f64))
                    .flatten()
            })
        })
        .unwrap_or(DEFAULT_ENCUMBRANCE_CAP) as f32;
    // FIXME: This hard-codes the game's carry-capacity formula: base 11; weight offset baseline 15, clamp [-60, 0], factor 0.1; sickness factor 0.025; hunger <= 100 adds 1.5 and < 40 subtracts 1.5; thirst < 40 subtracts 1; STR/RES each scale by 0.5.
    let base_capacity = 11.0 + (weight_offset + 15.0).clamp(-60.0, 0.0) * 0.1 - sickness * 0.025
        + if hunger <= 100.0 { 1.5 } else { 0.0 }
        - if hunger < 40.0 { 1.5 } else { 0.0 }
        - if thirst < 40.0 { 1.0 } else { 0.0 }
        + (strength * 0.5).min(resistance * 0.5);
    let capacity = base_capacity * run_cap;
    if !capacity.is_finite() || capacity <= 0.0 || !run_cap.is_finite() {
        return None;
    }

    let all = items(data);
    let mut weight = 0.0;
    let mut complete = data.get("items").is_some_and(Value::is_array);
    for group in group_items(data) {
        let Some(&root_index) = group.indices.first() else {
            continue;
        };
        let root = all.get(root_index)?;
        let root_id = root.get("id").and_then(Value::as_str).unwrap_or("");
        let Some(root_stats) = stats.get(root_id) else {
            complete = false;
            continue;
        };
        let root_weight = item_weights(data, root_index, stats);
        complete &= root_weight.game_complete;
        if let Some(value) = root_weight.game {
            weight += value;
        }
        if group.indices.len() <= 1 {
            continue;
        }
        let Some(multiplier) = root_stats.container_encumbrance else {
            complete = false;
            continue;
        };
        for &index in group.indices.iter().skip(1) {
            let Some(item) = all.get(index) else {
                complete = false;
                continue;
            };
            let id = item.get("id").and_then(Value::as_str).unwrap_or("");
            if !stats.contains_key(id) {
                complete = false;
                continue;
            }
            let item_weight = item_weights(data, index, stats);
            complete &= item_weight.game_complete;
            if let Some(value) = item_weight.game {
                weight += value * multiplier;
            }
        }
    }
    weight.is_finite().then_some(CarryLoad {
        weight,
        capacity,
        complete,
    })
}

fn item_condition(item: &Value) -> f32 {
    // FIXME: A missing item condition is hard-coded as full condition (1.0).
    item.get("condition")
        .and_then(Value::as_f64)
        .map_or(1.0, |value| value as f32)
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
    // FIXME: This hard-codes new-item defaults: condition 1.0, no wear slot, not favourited, empty label, and swatch 0.
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
    // FIXME: Healing hard-codes full skin/muscle health (100), zero pain/bleeding/infection/blood/shrapnel, and clears the listed injury flags.
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
    // FIXME: These post-heal targets are hard-coded: blood/oxygen/respiration/consciousness/brain health 100, heart rate 70, pressure 120, temperature 37; listed acute-condition values are 0 and forced fibrillation/embolism flags are false.
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
    // FIXME: This hard-codes filled needs to 100 and dirtiness/wetness to 0.
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
    // FIXME: The game skill-level range [0, 99] and `<KEY>From10 = level - 10` rule are hard-coded here.
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

/// Gives `skills` the starting levels and gain multipliers of a character,
/// with experience at the start of each level (port of `Skills.Setup`).
pub fn setup_character_skills(skills: &mut Map<String, Value>, character: usize) {
    let Some(def) = schema::CHARACTERS.get(character) else {
        return;
    };
    for (index, skill) in schema::SKILLS.iter().enumerate() {
        let key = skill.key;
        set_skill_level(skills, key, def.base_skills[index]);
        let min = get_f64(skills, &format!("min{key}")).unwrap_or(0.0);
        set_f64(skills, &format!("exp{key}"), min);
        set_f64(
            skills,
            &format!("{key}GainMult"),
            def.gain_multipliers[index],
        );
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

    fn item_stats(weight: f32, container_encumbrance: Option<f32>) -> ItemStats {
        ItemStats {
            category: "container".to_owned(),
            weight,
            scale_weight_with_condition: false,
            wearable: false,
            wear_slot: None,
            armor: None,
            insulation: None,
            item_tags: Default::default(),
            container_capacity: None,
            container_max_item_weight: None,
            container_encumbrance,
            container_tag_restriction: None,
            liquid_capacity: None,
            only_hold_in_hands: false,
            wearable_can_be_held: false,
            desired_wear_limb: None,
            destroy_at_zero_condition: false,
        }
    }

    #[test]
    fn groups_follow_holder_then_contents() {
        let g = group_items(&doc());
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].location, Location::Slot(5));
        assert_eq!(g[1].indices, vec![2, 3]);
    }

    #[test]
    fn carry_load_uses_container_multiplier_and_game_capacity_formula() {
        let data = json!({
            "body": {
                "weightOffset": 0.0,
                "sicknessAmount": 0.0,
                "hunger": 100.0,
                "thirst": 100.0,
                "skills": {"STRFrom10": 4.0, "RESFrom10": 2.0}
            },
            "runSettings": [],
            "items": [
                {"id": "bag", "slot": 0, "condition": 1.0},
                {"id": "rock", "slot": 0, "condition": 1.0}
            ]
        });
        let stats = HashMap::from([
            ("bag".to_owned(), item_stats(2.0, Some(0.25))),
            ("rock".to_owned(), item_stats(4.0, None)),
        ]);
        let load = carry_load(&data, &stats).unwrap();
        assert_eq!(load.weight, 3.0);
        assert_eq!(load.capacity, 13.5);
        assert!(load.complete);
    }

    #[test]
    fn container_space_uses_unmodified_contents_and_reports_overflow() {
        let data = json!({
            "items": [
                {"id": "bag", "slot": 0},
                {"id": "rock", "slot": 0},
                {"id": "tool", "slot": 0}
            ]
        });
        let mut bag = item_stats(1.0, Some(0.25));
        bag.container_capacity = Some(8.0);
        let mut stats = HashMap::from([
            ("bag".to_owned(), bag.clone()),
            ("rock".to_owned(), item_stats(2.0, None)),
            ("tool".to_owned(), item_stats(3.0, None)),
        ]);

        let space = container_space(&data, 0, &stats).unwrap();
        assert_eq!(space.used, 5.0);
        assert_eq!(space.remaining, 3.0);
        assert_eq!(space.over_capacity, 0.0);
        assert!(space.complete);

        bag.container_capacity = Some(4.0);
        stats.insert("bag".to_owned(), bag);
        let space = container_space(&data, 0, &stats).unwrap();
        assert_eq!(space.remaining, 0.0);
        assert_eq!(space.over_capacity, 1.0);

        stats.remove("tool");
        let space = container_space(&data, 0, &stats).unwrap();
        assert!(!space.complete);
    }

    #[test]
    fn carry_load_does_not_claim_completeness_for_missing_item_data() {
        let data = json!({
            "body": {
                "weightOffset": 0.0,
                "sicknessAmount": 0.0,
                "hunger": 100.0,
                "thirst": 100.0,
                "skills": {"STRFrom10": 4.0, "RESFrom10": 2.0}
            },
            "items": [{"id": "unknown", "slot": 0}]
        });
        let load = carry_load(&data, &HashMap::new()).unwrap();
        assert_eq!(load.weight, 0.0);
        assert!(!load.complete);
    }

    #[test]
    fn liquid_weight_uses_volume_and_keeps_game_and_physical_load_separate() {
        let data = json!({
            "body": {"weightOffset": 0.0, "sicknessAmount": 0.0, "hunger": 100.0,
                     "thirst": 100.0, "skills": {"STRFrom10": 4.0, "RESFrom10": 2.0}},
            "items": [
                {"id": "bag", "slot": 0},
                {"id": "bottle", "slot": 0, "condition": 1.0}
            ],
            "itemComponents": [{}, {"WaterContainerItem, Assembly-CSharp, Version=0.0.0.0": {"stack": [
                {"liquidId": "water", "amount": 100.0},
                {"liquidId": "other", "amount": 60.0}
            ]}}]
        });
        let mut bottle = item_stats(1.25, None);
        bottle.scale_weight_with_condition = true;
        bottle.liquid_capacity = Some(500.0);
        let stats = HashMap::from([
            ("bag".to_owned(), item_stats(2.0, Some(0.25))),
            ("bottle".to_owned(), bottle),
        ]);
        let weights = item_weights(&data, 1, &stats);
        assert!((weights.game.unwrap() - 0.468).abs() < 0.00001);
        assert!((weights.physical.unwrap() - 0.26).abs() < 0.00001);
        assert!((weights.liquid.unwrap() - 0.16).abs() < 0.00001);
        let game = carry_load(&data, &stats).unwrap();
        assert!((game.weight - 2.117).abs() < 0.00001);
        assert!(game.complete);
        let physical = physical_load(&data, &stats);
        assert!((physical.weight - 2.26).abs() < 0.00001);
        assert!(physical.complete);
    }

    #[test]
    fn empty_overfilled_and_invalid_liquid_data_are_handled() {
        let mut bottle = item_stats(1.25, None);
        bottle.scale_weight_with_condition = true;
        bottle.liquid_capacity = Some(500.0);
        let stats = HashMap::from([("bottle".to_owned(), bottle)]);
        let mut data = json!({
            "items": [{"id": "bottle", "slot": 0, "condition": 0.4}],
            "itemComponents": [{"WaterContainerItem": {"stack": []}}]
        });
        let empty = item_weights(&data, 0, &stats);
        assert!((empty.game.unwrap() - 0.1).abs() < 0.00001);
        assert!((empty.physical.unwrap() - 0.1).abs() < 0.00001);
        data["itemComponents"][0]["WaterContainerItem"]["stack"] = json!([{"amount": 750.0}]);
        let overfilled = item_weights(&data, 0, &stats);
        assert_eq!(overfilled.game, Some(1.25));
        assert!((overfilled.physical.unwrap() - 0.85).abs() < 0.00001);
        data["itemComponents"][0]["WaterContainerItem"]["stack"] = json!([{"amount": -1}]);
        let invalid = item_weights(&data, 0, &stats);
        assert!(!invalid.game_complete);
        assert!(invalid.physical.is_none());
        assert!(!physical_load(&data, &stats).complete);
        data["itemComponents"] = json!([{}]);
        assert!(!item_weights(&data, 0, &stats).game_complete);
        assert!(!physical_load(&data, &stats).complete);
        assert!(!physical_load(&json!({}), &stats).complete);
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

    #[test]
    fn character_setup_uses_the_game_starting_skills() {
        let mut d = doc();
        let skills = d["body"]["skills"].as_object_mut().unwrap();
        setup_character_skills(skills, 2);
        assert_eq!(skills["STR"], json!(17));
        assert_eq!(skills["minSTR"], json!(schema::experience_for_level(17)));
        assert_eq!(
            skills["expSTR"],
            json!(schema::experience_for_level(17) as f64)
        );
        assert_eq!(skills["INT"], json!(0));
        assert_eq!(skills["INTGainMult"], json!(0.5));
        assert_eq!(skills["STRGainMult"], json!(1.0));
    }
}
