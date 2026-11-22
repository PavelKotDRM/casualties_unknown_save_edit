//! Merchant stock probabilities from TraderScript.GenerateInventory.

use std::collections::HashMap;

use super::ItemStats;

const CATEGORIES: [&str; 8] = [
    "medical",
    "food",
    "water",
    "tool",
    "drug",
    "container",
    "utility",
    "custom",
];
const AMMUNITION: [&str; 3] = ["smallmagazine", "riflemagazine", "boxof12gauge"];
const WEAPONS: [&str; 3] = ["pistol", "rifle", "shotgun"];

pub fn character(name: &str) -> Option<u8> {
    match name {
        "trader1" => Some(0),
        "trader2" => Some(1),
        "trader3" => Some(2),
        _ => None,
    }
}

/// Chance an item appears in the merchant's generated inventory.
pub fn stock_probability(
    item_id: &str,
    items: &HashMap<String, ItemStats>,
    trader_item_amount: f32,
    character: u8,
) -> Option<f64> {
    if !trader_item_amount.is_finite() || trader_item_amount < 0.0 || character > 2 {
        return None;
    }

    let category_probability = category_probability(item_id, items).unwrap_or_default();
    let special_probability = match character {
        1 if AMMUNITION.contains(&item_id) => 1.0 - (2.0_f64 / 3.0).powi(2),
        1 if WEAPONS.contains(&item_id) => 1.0 / 3.0,
        _ => 0.0,
    };
    if category_probability == 0.0 && special_probability == 0.0 {
        return None;
    }

    let list_miss_probability = (2..9)
        .map(|random_count| {
            let count = rounded_count(random_count, trader_item_amount, character);
            (1.0 - category_probability).powi(count)
        })
        .sum::<f64>()
        / 7.0;
    let regular_stock_probability = 1.0 - list_miss_probability.powi(3);
    Some(1.0 - (1.0 - regular_stock_probability) * (1.0 - special_probability))
}

fn category_probability(item_id: &str, items: &HashMap<String, ItemStats>) -> Option<f64> {
    let item = items.get(item_id)?;
    if !tradeable(item) || !CATEGORIES.contains(&item.category.as_str()) {
        return None;
    }

    let mut pool_sizes = HashMap::<&str, usize>::new();
    let mut tradeable_counts = HashMap::<&str, usize>::new();
    for candidate in items.values() {
        if !CATEGORIES.contains(&candidate.category.as_str()) {
            continue;
        }
        *pool_sizes.entry(&candidate.category).or_default() += 1;
        if tradeable(candidate) {
            *tradeable_counts.entry(&candidate.category).or_default() += 1;
        }
    }

    let successful_category_weight: f64 = CATEGORIES
        .iter()
        .filter_map(|category| {
            let pool_size = pool_sizes.get(category)?;
            let tradeable_count = tradeable_counts.get(category).copied().unwrap_or_default();
            Some(tradeable_count as f64 / *pool_size as f64)
        })
        .sum();
    let pool_size = *pool_sizes.get(item.category.as_str())?;
    (successful_category_weight > 0.0)
        .then_some((1.0 / pool_size as f64) / successful_category_weight)
}

fn tradeable(item: &ItemStats) -> bool {
    item.trade_value > 0.0 && !item.untradeable
}

fn rounded_count(random_count: i32, amount: f32, character: u8) -> i32 {
    let count = round_to_int(random_count as f32 * amount);
    if character == 2 {
        round_to_int(count as f32 * 0.66)
    } else {
        count
    }
}

fn round_to_int(value: f32) -> i32 {
    let floor = value.floor();
    let fraction = value - floor;
    if fraction < 0.5 {
        floor as i32
    } else if fraction > 0.5 || (floor as i32) % 2 != 0 {
        floor as i32 + 1
    } else {
        floor as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn item(category: &str, trade_value: f32, untradeable: bool) -> ItemStats {
        ItemStats {
            category: category.into(),
            weight: 0.0,
            trade_value,
            untradeable,
            scale_weight_with_condition: false,
            wearable: false,
            wear_slot: None,
            armor: None,
            insulation: None,
            item_tags: Default::default(),
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

    fn items() -> HashMap<String, ItemStats> {
        let mut items = HashMap::new();
        for index in 0..10 {
            items.insert(format!("medical{index}"), item("medical", 1.0, false));
            items.insert(format!("food{index}"), item("food", 1.0, false));
        }
        items.insert("untradeable".into(), item("medical", 1.0, true));
        items.insert("no_value".into(), item("medical", 0.0, false));
        items
    }

    #[test]
    fn stock_chance_uses_random_draws_and_trader_specific_counts() {
        let items = items();
        let trader_one = stock_probability("medical0", &items, 1.0, 0).unwrap();
        let trader_three = stock_probability("medical0", &items, 1.0, 2).unwrap();
        assert!(trader_one > 0.0 && trader_one < 1.0);
        assert!(trader_three > 0.0 && trader_three < trader_one);
        assert_eq!(stock_probability("untradeable", &items, 1.0, 0), None);
        assert_eq!(stock_probability("no_value", &items, 1.0, 0), None);
    }

    #[test]
    fn second_trader_has_random_special_weapon_and_ammunition_stock() {
        let items = items();
        let base = stock_probability("smallmagazine", &items, 1.0, 0);
        assert_eq!(
            stock_probability("smallmagazine", &items, 1.0, 1),
            Some(1.0 - (1.0 - base.unwrap_or_default()) * (2.0_f64 / 3.0).powi(2))
        );
        assert_eq!(character("trader1"), Some(0));
        assert_eq!(character("trader2"), Some(1));
        assert_eq!(character("trader3"), Some(2));
        assert_eq!(character("crate"), None);
    }
}
