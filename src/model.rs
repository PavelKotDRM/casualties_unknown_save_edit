//! Structural operations on the save document.
//!
//! These functions encapsulate edits that must keep several parts of the
//! save consistent, e.g. the parallel `items` / `itemComponents` arrays, or
//! skill levels and their experience boundaries. They are UI-independent and
//! unit tested.

mod ammunition;
mod balance;
mod health;
mod inventory;
mod item_state;
mod load;
pub mod placement;
mod protection;
mod skills;

pub use ammunition::{AmmunitionStatus, ChamberState, ammunition_status, reload_ammunition};
pub use balance::{BalanceRule, balance_containers};
pub use health::{fill_needs, heal_limb, heal_limbs, restore_vitals};
pub use inventory::{
    ItemGroup, Location, add_item, duplicate_item, group_items, move_to_location,
    move_within_group, new_item, new_item_components, remove_item,
};
pub use item_state::{
    ItemContentState, battery_capacity, item_content_state, remaining_uses, repair_and_charge_all,
    set_battery_charge, set_remaining_uses, synchronize_item_state,
};
pub use load::{
    CarryLoad, ContainerSpace, ItemWeights, PhysicalLoad, carry_load, container_space,
    item_weights, physical_load,
};
pub use protection::{LimbProtection, Protection, calculate_insulation, calculate_protection};
pub use skills::{set_skill_level, setup_character_skills, skill_progress};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{game_data::ItemStats, schema};
    use serde_json::{Value, json};
    use std::collections::HashMap;

    fn runtime_fixture() -> crate::game_data::runtime::TestMetadata {
        use crate::game_data::{metadata::Metadata, program::ScalarProgram};
        let limb_defaults = json!({
            "skinHealth": 80.0, "muscleHealth": 90.0, "pain": 0.0,
            "bleedAmount": 0.0, "infectionAmount": 0.0, "furBloodAmount": 0.0,
            "dislocated": false, "broken": false, "infected": false,
            "splinted": false, "blockedBleeding": false, "shrapnel": 0,
            "dismembered": false
        })
        .as_object()
        .unwrap()
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
        let item_defaults =
            json!({"condition": 0.75, "favourited": false, "label": "", "swatch": 7})
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
        crate::game_data::runtime::scoped_for_test(Metadata {
            limb_defaults,
            item_defaults,
            minimum_item_weight: Some(0.1),
            carry_capacity: Some(ScalarProgram::test_linear(Some("hunger"), 0.1, 2.0)),
            experience: Some(ScalarProgram::test_linear(None, 20.0, 0.0)),
            skill_derived: [(
                "STRFrom10".into(),
                ScalarProgram::test_linear(Some("STR"), 1.0, -7.0),
            )]
            .into(),
            skills: ["STR", "RES", "INT"]
                .map(|key| schema::SkillDef {
                    key,
                    name: key,
                    locale_key: key,
                    help: "",
                })
                .into(),
            characters: vec![
                schema::CharacterDef {
                    name: "Synthetic character",
                    base_skills: [17, 4, 0],
                    gain_multipliers: [1.0, 1.0, 0.5],
                    height_multiplier: Some(1.0),
                };
                3
            ],
            ..Default::default()
        })
    }

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

    fn balancing_fixture(
        capacities: [f32; 2],
        weights: &[f32],
    ) -> (Value, HashMap<String, ItemStats>) {
        let mut stats = HashMap::new();
        for (index, capacity) in capacities.into_iter().enumerate() {
            let mut bag = item_stats(1.0, Some(if index == 0 { 0.25 } else { 1.0 }));
            bag.container_capacity = Some(capacity);
            bag.container_max_item_weight = Some(capacity);
            bag.container_tag_restriction = Some(Default::default());
            stats.insert(format!("bag{index}"), bag);
        }
        let mut data = json!({"items": [
            {"id": "bag0", "slot": 0, "condition": 1.0},
            {"id": "bag1", "slot": 1, "condition": 1.0}
        ], "itemComponents": [{"marker": "bag0"}, {"marker": "bag1"}]});
        for (index, &weight) in weights.iter().enumerate() {
            let id = format!("cargo{index}");
            stats.insert(id.clone(), item_stats(weight, None));
            let item = json!({"id": id, "slot": 1, "condition": 1.0, "favourited": false});
            add_item(&mut data, item, json!({"marker": id})).unwrap();
        }
        (data, stats)
    }

    #[test]
    fn balancing_finds_exact_pack_instead_of_greedy_and_preserves_components() {
        let (mut data, stats) = balancing_fixture([10.0, 30.0], &[6.0, 5.0, 5.0]);
        assert_eq!(
            balance_containers(&mut data, &stats, BalanceRule::CarryWeight, true),
            Ok(2)
        );
        let groups = group_items(&data);
        let first = &groups[0];
        let stored: Vec<_> = first
            .indices
            .iter()
            .skip(1)
            .map(|&i| data["items"][i]["id"].as_str().unwrap())
            .collect();
        assert_eq!(stored, ["cargo1", "cargo2"]);
        for (item, component) in data["items"]
            .as_array()
            .unwrap()
            .iter()
            .zip(data["itemComponents"].as_array().unwrap())
        {
            assert_eq!(item["id"], component["marker"]);
        }
        let balanced = data.clone();
        assert_eq!(
            balance_containers(&mut data, &stats, BalanceRule::CarryWeight, true),
            Ok(0)
        );
        assert_eq!(data, balanced);
    }

    #[test]
    fn balancing_respects_favourites_tags_and_per_item_limits() {
        let (mut data, mut stats) = balancing_fixture([20.0, 30.0], &[6.0, 5.0, 4.0, 3.0]);
        stats.get_mut("bag0").unwrap().container_tag_restriction = Some(["medical".into()].into());
        stats.get_mut("bag0").unwrap().container_max_item_weight = Some(5.0);
        for id in ["cargo0", "cargo1", "cargo3"] {
            stats
                .get_mut(id)
                .unwrap()
                .item_tags
                .insert("medical".into());
        }
        data["items"][3]["favourited"] = json!(true);
        assert_eq!(
            balance_containers(&mut data, &stats, BalanceRule::CarryWeight, true),
            Ok(1)
        );
        let first = &group_items(&data)[0];
        assert_eq!(first.indices.len(), 2);
        assert_eq!(data["items"][first.indices[1]]["id"], "cargo3");
    }

    #[test]
    fn balancing_keeps_large_specialized_contents_without_favourite_flags() {
        let (mut original, mut stats) = balancing_fixture([10.0, 30.0], &[6.0, 5.0, 5.0]);
        let mut holder = item_stats(1.0, Some(0.9));
        holder.container_capacity = Some(3.0);
        holder.container_max_item_weight = Some(0.1);
        holder.container_tag_restriction = Some(["ammo".into()].into());
        stats.insert("specialized".into(), holder);
        let mut round = item_stats(0.03, None);
        round.item_tags.insert("ammo".into());
        stats.insert("round".into(), round);
        add_item(
            &mut original,
            json!({"id":"specialized","slot":2,"condition":1.0}),
            json!({}),
        )
        .unwrap();
        for marker in 0..91 {
            add_item(
                &mut original,
                json!({"id":"round","slot":2,"condition":1.0,"favourited":false,"label":marker}),
                json!({"marker":marker}),
            )
            .unwrap();
        }
        for rule in BalanceRule::ALL {
            let mut data = original.clone();
            balance_containers(&mut data, &stats, rule, false).unwrap();
            let group = group_items(&data)
                .into_iter()
                .find(|g| g.location == Location::Slot(2))
                .unwrap();
            assert_eq!(group.indices.len(), 92);
            for (marker, &index) in group.indices.iter().skip(1).enumerate() {
                assert_eq!(data["items"][index]["id"], "round");
                assert_eq!(data["items"][index]["label"], marker);
                assert_eq!(data["itemComponents"][index]["marker"], marker);
            }
        }
    }

    #[test]
    fn balancing_can_repair_overfilled_specialized_storage() {
        let (mut data, mut stats) = balancing_fixture([10.0, 10.0], &[6.0, 5.0, 4.0]);
        stats.get_mut("bag1").unwrap().container_tag_restriction = Some(["medical".into()].into());
        for id in ["cargo0", "cargo1", "cargo2"] {
            stats
                .get_mut(id)
                .unwrap()
                .item_tags
                .insert("medical".into());
        }
        balance_containers(&mut data, &stats, BalanceRule::CarryWeight, true).unwrap();
        for group in group_items(&data) {
            assert_eq!(
                container_space(&data, group.indices[0], &stats)
                    .unwrap()
                    .over_capacity,
                0.0
            );
        }
    }

    #[test]
    fn balancing_fractional_three_bin_objectives_match_exhaustive_search() {
        let weights = [1.3525_f32, 0.29875, 0.3, 0.3, 0.1];
        let (mut original, mut stats) = balancing_fixture([2.0, 2.4], &weights);
        let mut bag = item_stats(1.0, Some(0.6));
        bag.container_capacity = Some(1.8);
        bag.container_max_item_weight = Some(1.0);
        bag.container_tag_restriction = Some(Default::default());
        stats.insert("bag2".into(), bag);
        stats.insert("fixed".into(), item_stats(0.7, None));
        add_item(
            &mut original,
            json!({"id":"bag2","slot":2,"condition":1.0}),
            json!({}),
        )
        .unwrap();
        add_item(
            &mut original,
            json!({"id":"fixed","slot":2,"condition":1.0,"favourited":true}),
            json!({}),
        )
        .unwrap();
        let capacities = [f64::from(2.0_f32), f64::from(2.4_f32), f64::from(1.8_f32)];
        for rule in BalanceRule::ALL {
            let score = |loads: &[f64; 3]| match rule {
                BalanceRule::CarryWeight => {
                    loads[0] * 0.25 + loads[1] + loads[2] * f64::from(0.6_f32)
                }
                BalanceRule::EqualWeight => loads.iter().map(|w| w * w).sum(),
                BalanceRule::EqualFill => {
                    let fills = std::array::from_fn::<_, 3, _>(|i| loads[i] / capacities[i]);
                    let mean = fills.iter().sum::<f64>() / 3.0;
                    fills.iter().map(|fill| (fill - mean).powi(2)).sum()
                }
            };
            let mut optimum = f64::INFINITY;
            for mut assignment in 0..3_usize.pow(weights.len() as u32) {
                let mut loads = [0.0, 0.0, f64::from(0.7_f32)];
                let mut valid = true;
                for weight in weights {
                    let bin = assignment % 3;
                    assignment /= 3;
                    loads[bin] += f64::from(weight);
                    valid &= bin != 2 || weight <= 1.0;
                }
                if valid
                    && loads
                        .iter()
                        .zip(capacities)
                        .all(|(&load, capacity)| load <= capacity)
                {
                    optimum = optimum.min(score(&loads));
                }
            }
            let mut data = original.clone();
            balance_containers(&mut data, &stats, rule, true).unwrap();
            let loads: Vec<f64> = group_items(&data)
                .iter()
                .map(|group| {
                    group
                        .indices
                        .iter()
                        .skip(1)
                        .map(|&index| f64::from(item_weights(&data, index, &stats).game.unwrap()))
                        .sum()
                })
                .collect();
            assert!(
                (score(&[loads[0], loads[1], loads[2]]) - optimum).abs() < 1e-10,
                "{rule:?}"
            );
        }
    }

    #[test]
    fn balancing_rules_have_distinct_verified_objectives() {
        let (data, stats) = balancing_fixture([10.0, 20.0], &[2.0, 2.0, 2.0, 2.0, 2.0, 2.0]);
        for (rule, expected) in [
            (BalanceRule::EqualWeight, 6.0),
            (BalanceRule::EqualFill, 4.0),
        ] {
            let mut data = data.clone();
            balance_containers(&mut data, &stats, rule, true).unwrap();
            assert_eq!(container_space(&data, 0, &stats).unwrap().used, expected);
        }
    }

    #[test]
    fn balancing_rules_match_exhaustive_feasible_distributions() {
        let weights = [6.0_f32, 5.0, 4.0, 3.0];
        let (original, stats) = balancing_fixture([10.0, 18.0], &weights);
        for rule in BalanceRule::ALL {
            let score = |first: f64, second: f64| match rule {
                BalanceRule::CarryWeight => first * 0.25 + second,
                BalanceRule::EqualWeight => first.powi(2) + second.powi(2),
                BalanceRule::EqualFill => (first / 10.0 - second / 18.0).powi(2),
            };
            let mut optimum = f64::INFINITY;
            for mask in 0..1 << weights.len() {
                let first: f64 = weights
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, weight)| f64::from(*weight))
                    .sum();
                let second = 18.0 - first;
                if first <= 10.0 && second <= 18.0 {
                    optimum = optimum.min(score(first, second));
                }
            }
            let mut data = original.clone();
            balance_containers(&mut data, &stats, rule, true).unwrap();
            let groups = group_items(&data);
            let loads: Vec<_> = groups
                .iter()
                .map(|group| {
                    f64::from(
                        container_space(&data, group.indices[0], &stats)
                            .unwrap()
                            .used,
                    )
                })
                .collect();
            assert!(
                (score(loads[0], loads[1]) - optimum).abs() < 1e-8,
                "{rule:?}"
            );
        }
    }

    #[test]
    fn balancing_rejects_incomplete_weights_and_unaligned_components_without_mutating() {
        let (mut data, mut stats) = balancing_fixture([10.0, 20.0], &[4.0]);
        stats.remove("cargo0");
        let before = data.clone();
        assert!(balance_containers(&mut data, &stats, BalanceRule::CarryWeight, true).is_err());
        assert_eq!(data, before);
        data["itemComponents"] = json!([]);
        let before = data.clone();
        assert!(balance_containers(&mut data, &stats, BalanceRule::CarryWeight, true).is_err());
        assert_eq!(data, before);
    }

    #[test]
    fn balancing_repairs_overcapacity_and_reports_impossible_packing_without_mutating() {
        let (mut data, stats) = balancing_fixture([10.0, 10.0], &[6.0, 5.0, 4.0]);
        balance_containers(&mut data, &stats, BalanceRule::CarryWeight, true).unwrap();
        for group in group_items(&data) {
            assert_eq!(
                container_space(&data, group.indices[0], &stats)
                    .unwrap()
                    .over_capacity,
                0.0
            );
        }
        let (mut data, stats) = balancing_fixture([10.0, 10.0], &[8.0, 8.0, 4.0]);
        let before = data.clone();
        assert!(
            balance_containers(&mut data, &stats, BalanceRule::CarryWeight, true)
                .unwrap_err()
                .contains("No valid distribution")
        );
        assert_eq!(data, before);
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
        let _metadata = runtime_fixture();
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
        assert_eq!(load.capacity, 12.0);
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
        let _metadata = runtime_fixture();
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
        let _metadata = runtime_fixture();
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
        let _metadata = runtime_fixture();
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
        let i = add_item(&mut d, json!({"id": "lantern", "slot": 3}), json!({})).unwrap();
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
        let _metadata = runtime_fixture();
        let mut d = doc();
        heal_limbs(&mut d, false).unwrap();
        assert_eq!(d["limbs"][0]["skinHealth"], json!(80.0));
        assert_eq!(d["limbs"][0]["broken"], json!(false));
        assert_eq!(d["limbs"][0]["shrapnel"], json!(0));
        assert_eq!(d["limbs"][1]["skinHealth"], json!(0.0));
        heal_limbs(&mut d, true).unwrap();
        assert_eq!(d["limbs"][1]["dismembered"], json!(false));
    }

    #[test]
    fn skill_level_change_updates_boundaries_and_keeps_progress() {
        let _metadata = runtime_fixture();
        let mut d = doc();
        let skills = d["body"]["skills"].as_object_mut().unwrap();
        set_skill_level(skills, "STR", 14).unwrap();
        assert_eq!(skills["STR"], json!(14));
        assert_eq!(skills["minSTR"], json!(280));
        assert_eq!(skills["maxSTR"], json!(300));
        assert_eq!(skills["STRFrom10"], json!(7.0));
        let (_, p) = skill_progress(skills, "STR");
        assert!((p - 0.5).abs() < 1e-3);
    }

    #[test]
    fn character_setup_uses_the_game_starting_skills() {
        let _metadata = runtime_fixture();
        let mut d = doc();
        let skills = d["body"]["skills"].as_object_mut().unwrap();
        setup_character_skills(skills, 2).unwrap();
        assert_eq!(skills["STR"], json!(17));
        assert_eq!(
            skills["minSTR"],
            json!(schema::experience_for_level(17).unwrap())
        );
        assert_eq!(
            skills["expSTR"],
            json!(schema::experience_for_level(17).unwrap() as f64)
        );
        assert_eq!(skills["INT"], json!(0));
        assert_eq!(skills["INTGainMult"], json!(0.5));
        assert_eq!(skills["STRGainMult"], json!(1.0));
    }

    #[test]
    fn unavailable_runtime_data_never_fabricates_an_edit() {
        let _metadata = crate::game_data::runtime::scoped_for_test(Default::default());
        let mut data = doc();
        let before = data.clone();
        assert!(heal_limbs(&mut data, true).is_err());
        assert!(restore_vitals(&mut data).is_err());
        assert!(fill_needs(&mut data).is_err());
        assert_eq!(data, before);
        let skills = data["body"]["skills"].as_object_mut().unwrap();
        assert!(set_skill_level(skills, "STR", 14).is_err());
        assert_eq!(data, before);
        assert!(new_item("example", &Location::Slot(0)).is_err());
    }

    #[test]
    fn new_items_and_skill_edits_use_runtime_defaults_not_old_constants() {
        let _metadata = runtime_fixture();
        let item = new_item("example", &Location::Slot(1)).unwrap();
        assert_eq!(item["condition"], 0.75);
        assert_eq!(item["swatch"], 7);
        assert_eq!(item["slot"], 1);
        let mut skills = serde_json::Map::new();
        set_skill_level(&mut skills, "STR", 100).unwrap();
        assert_eq!(skills["STR"], 100);
        let before = skills.clone();
        assert!(set_skill_level(&mut skills, "STR", -1).is_err());
        assert_eq!(skills, before);
    }
}
