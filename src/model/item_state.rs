//! Native dependencies between item condition and saved contents.

use serde_json::{Value, json};

use crate::game_data::{component_key, effects::ConditionUse, metadata::Metadata};
use crate::json;

#[derive(Debug, Default, PartialEq)]
pub struct ItemContentState {
    pub condition: Option<f64>,
    pub battery_capacity: Option<f64>,
}

pub fn battery_capacity(id: &str, battery: &str, metadata: &Metadata) -> Result<f64, String> {
    if battery.is_empty() {
        return Ok(0.0);
    }
    let charge = metadata
        .battery_charges
        .get(battery)
        .copied()
        .filter(|charge| charge.is_finite() && *charge > 0.0)
        .ok_or_else(|| format!("Battery capacity for {battery} is unavailable"))?;
    let limit = metadata
        .battery_limits
        .get(id)
        .copied()
        .filter(|limit| limit.is_finite() && *limit > 0.0)
        .ok_or_else(|| format!("Battery compatibility for {id} is unavailable"))?;
    if charge > limit {
        return Err(format!("{battery} exceeds the battery capacity of {id}"));
    }
    Ok(f64::from(charge))
}

pub fn item_content_state(
    item: &Value,
    components: &Value,
    metadata: &Metadata,
) -> Result<ItemContentState, String> {
    let id = item["id"].as_str().ok_or("Invalid item id")?;
    let saved = components.as_object().ok_or("Invalid item components")?;
    let defaults = metadata.item_components.get(id);
    let component = |owner| {
        let key = component_key(owner);
        saved
            .get(&key)
            .or_else(|| defaults.and_then(|value| value.get(&key)))
    };
    let mut state = ItemContentState::default();
    if let Some(water) = component("WaterContainerItem") {
        let capacity = metadata
            .items
            .iter()
            .find(|definition| definition.id == id)
            .map(|definition| definition.capacity)
            .filter(|capacity| capacity.is_finite() && *capacity > 0.0)
            .ok_or_else(|| format!("Liquid capacity for {id} is unavailable"))?;
        let stack = water["stack"].as_array().ok_or("Invalid liquid stack")?;
        let total = stack.iter().try_fold(0.0_f32, |total, entry| {
            let amount = entry["amount"]
                .as_f64()
                .filter(|amount| amount.is_finite() && *amount >= 0.0)
                .ok_or("Invalid liquid amount")? as f32;
            let total = total + amount;
            if !total.is_finite() {
                return Err("Liquid total is not finite");
            }
            Ok(total)
        })?;
        let condition = total / capacity;
        if !condition.is_finite() {
            return Err("Liquid condition is not finite".into());
        }
        state.condition = Some(f64::from(condition));
    }
    if let Some(battery) = component("BatteryItem") {
        let kind = battery["batteryType"]
            .as_str()
            .ok_or("Invalid battery type")?;
        state.battery_capacity = Some(battery_capacity(id, kind, metadata)?);
        if kind.is_empty() {
            state.condition = Some(0.0);
        }
    }
    Ok(state)
}

/// Validate first so invalid contents cannot partially change the item.
pub fn synchronize_item_state(
    item: &mut Value,
    components: &mut Value,
    metadata: &Metadata,
) -> Result<bool, String> {
    if !item.is_object() {
        return Err("Invalid item object".into());
    }
    let state = item_content_state(item, components, metadata)?;
    let mut changed = false;
    if let Some(condition) = state.condition
        && item["condition"].as_f64().map(|value| value as f32) != Some(condition as f32)
    {
        item["condition"] = json!(condition);
        changed = true;
    }
    if let Some(capacity) = state.battery_capacity
        && let Some(battery) = components.get_mut(component_key("BatteryItem"))
    {
        let object = battery.as_object_mut().ok_or("Invalid battery component")?;
        if object.get("maxCharge").and_then(Value::as_f64) != Some(capacity) {
            object.insert("maxCharge".into(), json!(capacity));
            changed = true;
        }
    }
    Ok(changed)
}

pub fn set_battery_charge(item: &mut Value, charge: f64, capacity: f64) -> Result<bool, String> {
    if !capacity.is_finite()
        || capacity <= 0.0
        || !charge.is_finite()
        || !(0.0..=capacity).contains(&charge)
    {
        return Err("Battery charge must be between zero and its maximum".into());
    }
    let object = item.as_object_mut().ok_or("Invalid item object")?;
    let condition = charge / capacity;
    if object.get("condition").and_then(Value::as_f64) == Some(condition) {
        return Ok(false);
    }
    json::set_f64(object, "condition", condition);
    Ok(true)
}

pub fn remaining_uses(condition: f32, rule: ConditionUse) -> Result<u32, String> {
    if !condition.is_finite()
        || !rule.cost.is_finite()
        || rule.cost <= 0.0
        || !rule.minimum.is_finite()
        || rule.minimum < 0.0
    {
        return Err("Invalid condition or item-use rule".into());
    }
    let mut remaining = condition;
    let mut count = 0;
    while remaining > 0.0
        && if rule.inclusive {
            remaining >= rule.minimum
        } else {
            remaining > rule.minimum
        }
    {
        let next = remaining - rule.cost;
        if next >= remaining || count >= 100_000 {
            return Err("Item-use count exceeds the calculation limit".into());
        }
        remaining = next;
        count += 1;
    }
    Ok(count)
}

pub fn set_remaining_uses(
    item: &mut Value,
    rule: ConditionUse,
    count: u32,
) -> Result<bool, String> {
    let condition = item["condition"].as_f64().ok_or("Invalid item condition")? as f32;
    let current = remaining_uses(condition, rule)?;
    if current == count {
        return Ok(false);
    }
    let full = remaining_uses(1.0, rule)?;
    if count > full {
        return Err(format!("This item holds at most {full} uses"));
    }
    let mut condition = if count == 0 { 0.0 } else { 1.0_f32 };
    if count > 0 {
        for _ in count..full {
            condition -= rule.cost;
        }
    }
    let object = item.as_object_mut().ok_or("Invalid item object")?;
    json::set_f64(object, "condition", f64::from(condition));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::ItemDef;

    fn metadata() -> Metadata {
        Metadata {
            items: vec![ItemDef {
                id: "drink",
                category: "",
                group: "",
                wear_slot: "",
                capacity: 500.0,
            }],
            battery_charges: [
                ("small".into(), 50.0),
                ("medium".into(), 100.0),
                ("large".into(), 300.0),
            ]
            .into(),
            battery_limits: [("light".into(), 100.0)].into(),
            ..Default::default()
        }
    }

    #[test]
    fn liquid_changes_drive_condition_without_clamping_or_changing_the_mixture() {
        let water = component_key("WaterContainerItem");
        let mut item = json!({"id": "drink", "condition": 1.0, "label": "keep"});
        let mut components = json!({&water: {"stack": [
            {"liquidId": "water", "amount": 100.0}, {"liquidId": "juice", "amount": 25.0}
        ]}, "other": {"keep": true}});
        let original = components.clone();
        assert!(synchronize_item_state(&mut item, &mut components, &metadata()).unwrap());
        assert_eq!(item["condition"], 0.25);
        assert_eq!(item["label"], "keep");
        assert_eq!(components, original);
        components[&water]["stack"][0]["amount"] = json!(600.0);
        synchronize_item_state(&mut item, &mut components, &metadata()).unwrap();
        assert_eq!(item["condition"], 1.25);
        components[&water]["stack"] = json!([]);
        synchronize_item_state(&mut item, &mut components, &metadata()).unwrap();
        assert_eq!(item["condition"], 0.0);
    }

    #[test]
    fn battery_identity_drives_capacity_and_removal_clears_charge() {
        let battery = component_key("BatteryItem");
        let mut item = json!({"id": "light", "condition": 0.6});
        let mut components = json!({&battery: {"batteryType": "small", "maxCharge": 100.0}});
        synchronize_item_state(&mut item, &mut components, &metadata()).unwrap();
        assert_eq!(components[&battery]["maxCharge"], 50.0);
        assert_eq!(item["condition"], 0.6);
        set_battery_charge(&mut item, 10.0, 50.0).unwrap();
        assert_eq!(item["condition"], 0.2);
        let before = item.clone();
        assert!(set_battery_charge(&mut item, 51.0, 50.0).is_err());
        assert!(set_battery_charge(&mut item, -1.0, 50.0).is_err());
        assert_eq!(item, before);
        components[&battery]["batteryType"] = json!("medium");
        synchronize_item_state(&mut item, &mut components, &metadata()).unwrap();
        assert_eq!(components[&battery]["maxCharge"], 100.0);
        assert_eq!(item["condition"], 0.2);
        components[&battery]["batteryType"] = json!("");
        synchronize_item_state(&mut item, &mut components, &metadata()).unwrap();
        assert_eq!(components[&battery]["maxCharge"], 0.0);
        assert_eq!(item["condition"], 0.0);
    }

    #[test]
    fn invalid_contents_and_incompatible_batteries_leave_state_unchanged() {
        for components in [
            json!({component_key("WaterContainerItem"): {"stack": [{"amount": -1}]}}),
            json!({component_key("BatteryItem"): {"batteryType": "large", "maxCharge": 1}}),
            json!({component_key("BatteryItem"): {"batteryType": "unknown", "maxCharge": 1}}),
        ] {
            let mut item = json!({"id": if components.get(component_key("WaterContainerItem")).is_some() {"drink"} else {"light"}, "condition": 0.7});
            let before = item.clone();
            let mut edited = components.clone();
            assert!(synchronize_item_state(&mut item, &mut edited, &metadata()).is_err());
            assert_eq!(item, before);
            assert_eq!(edited, components);
        }
    }

    #[test]
    fn uses_follow_native_subtraction_and_guards_in_both_directions() {
        let bread = ConditionUse {
            on_limb: false,
            cost: 0.34,
            minimum: 0.0,
            inclusive: false,
        };
        assert_eq!(remaining_uses(1.0, bread).unwrap(), 3);
        let cold_pack = ConditionUse {
            cost: 0.5,
            minimum: 0.5,
            inclusive: true,
            ..bread
        };
        assert_eq!(remaining_uses(0.49, cold_pack).unwrap(), 0);
        assert_eq!(remaining_uses(0.5, cold_pack).unwrap(), 1);
        let drain = ConditionUse {
            cost: 0.1,
            minimum: 0.1,
            inclusive: false,
            ..bread
        };
        assert_eq!(remaining_uses(0.1, drain).unwrap(), 0);
        assert_eq!(remaining_uses(1.0, drain).unwrap(), 9);
        let small_cost = ConditionUse {
            cost: 0.01,
            ..bread
        };
        assert_eq!(remaining_uses(1.0, small_cost).unwrap(), 101);
        for rule in [bread, cold_pack, drain, small_cost] {
            for count in 0..=remaining_uses(1.0, rule).unwrap() {
                let mut item = json!({"condition": 1.0, "other": 123});
                set_remaining_uses(&mut item, rule, count).unwrap();
                assert_eq!(
                    remaining_uses(item["condition"].as_f64().unwrap() as f32, rule).unwrap(),
                    count
                );
                assert_eq!(item["other"], 123);
            }
        }
        let mut item = json!({"condition": 0.5});
        let before = item.clone();
        assert!(set_remaining_uses(&mut item, bread, 4).is_err());
        assert_eq!(item, before);
    }

    #[test]
    fn missing_saved_components_use_defaults_without_materializing_them() {
        let mut metadata = metadata();
        metadata.item_components.insert(
            "drink".into(),
            json!({
                component_key("WaterContainerItem"): {"stack": [{"amount": 250.0}]}
            }),
        );
        let mut item = json!({"id": "drink", "condition": 1.0});
        let mut components = json!({});
        assert_eq!(
            item_content_state(&item, &components, &metadata)
                .unwrap()
                .condition,
            Some(0.5)
        );
        assert_eq!(item["condition"], 1.0);
        synchronize_item_state(&mut item, &mut components, &metadata).unwrap();
        assert_eq!(item["condition"], 0.5);
        assert_eq!(components, json!({}));
    }
}
