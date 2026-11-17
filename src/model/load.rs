//! Item weights, container capacity and carrying load.

use super::{group_items, inventory::items};
use crate::game_data::ItemStats;
use crate::game_data::runtime::metadata;
use serde_json::Value;
use std::collections::HashMap;

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
            game: weight.is_finite().then_some(weight),
            game_complete: weight.is_finite(),
            physical: weight.is_finite().then_some(weight),
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
            game: Some(item_stats.total_weight(item_condition(item)))
                .filter(|value| value.is_finite()),
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

/// Computes the game's encumbrance value from equipped items, container
/// contents, body state and run settings.
pub fn carry_load(data: &Value, stats: &HashMap<String, ItemStats>) -> Option<CarryLoad> {
    let body = data.get("body")?;
    let mut fields = HashMap::new();
    for (key, value) in body.as_object()? {
        if let Some(value) = value.as_f64() {
            fields.insert(key.clone(), value as f32);
        }
    }
    for (key, value) in body.get("skills")?.as_object()? {
        if let Some(value) = value.as_f64() {
            fields.insert(format!("skills.{key}"), value as f32);
        }
    }
    if let Some(preset) = metadata().presets.first() {
        for &(key, value) in preset.overrides {
            if let Some(value) = value.to_json().as_f64() {
                fields.insert(key.into(), value as f32);
            }
        }
    }
    if let Some(settings) = data.get("runSettings").and_then(Value::as_array) {
        for setting in settings {
            if let (Some(key), Some(value)) = (setting["Item1"].as_str(), setting["Item2"].as_f64())
            {
                fields.insert(key.into(), value as f32);
            }
        }
    }
    let capacity = metadata()
        .carry_capacity
        .as_ref()?
        .evaluate_fields(None, &fields)?;
    if !capacity.is_finite() || capacity <= 0.0 {
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
    item.get("condition")
        .or_else(|| metadata().item_defaults.get("condition"))
        .and_then(Value::as_f64)
        .map_or(f32::NAN, |value| value as f32)
}
