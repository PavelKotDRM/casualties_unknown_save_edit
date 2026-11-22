//! Limb armor and global clothing insulation, following the game's worn items.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::game_data::{ItemStats, metadata::Metadata};

use super::{Location, group_items};

#[derive(Debug, Clone, PartialEq)]
pub struct LimbProtection {
    pub divisor: f32,
    pub clothing: Vec<(String, f32)>,
    pub prosthetic: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Protection {
    pub limbs: Vec<LimbProtection>,
    pub clothing_insulation: f32,
    pub run_bonus: f32,
    pub character_bonus: f32,
}

fn setting(data: &Value, id: &str) -> Result<f32, String> {
    let settings = data["runSettings"]
        .as_array()
        .ok_or("Run settings are unavailable")?;
    let mut matches = settings
        .iter()
        .filter(|setting| setting["Item1"].as_str() == Some(id));
    let value = matches
        .next()
        .ok_or_else(|| format!("Run setting {id} is unavailable"))?;
    if matches.next().is_some() {
        return Err(format!("Duplicate run setting: {id}"));
    }
    value["Item2"]
        .as_f64()
        .or_else(|| value["Item2"].as_str().and_then(|value| value.parse().ok()))
        .filter(|value: &f64| value.is_finite())
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("Invalid run setting: {id}"))
}

/// Total insulation without and with the game's transient sleeping-bag bonus.
pub fn calculate_insulation(
    data: &Value,
    clothing: f32,
    metadata: &Metadata,
) -> Result<[f32; 2], String> {
    let rules = metadata
        .protection
        .as_ref()
        .ok_or("Protection metadata is unavailable")?;
    let program = rules
        .insulation
        .as_ref()
        .ok_or("Insulation formula is unavailable")?;
    let character = data["character"].as_u64().ok_or("Invalid character")? as f32;
    let weight = data["body"]
        .get("weightOffset")
        .or_else(|| metadata.body_defaults.get("weightOffset"))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or("Body weight offset is unavailable")? as f32;
    let mut fields = HashMap::from([
        ("charType".into(), character),
        ("clothingTemperature".into(), clothing),
        ("weightOffset".into(), weight),
        (
            "insulationmultiplier".into(),
            setting(data, "insulationmultiplier")?,
        ),
    ]);
    let mut values = [0.0; 2];
    for (sleeping, result) in values.iter_mut().enumerate() {
        fields.insert("usingSleepingBag".into(), sleeping as f32);
        *result = program
            .evaluate_fields(None, &fields)
            .ok_or("Cannot evaluate insulation formula")?;
    }
    Ok(values)
}

pub fn calculate_protection(
    data: &Value,
    stats: &HashMap<String, ItemStats>,
    metadata: &Metadata,
) -> Result<Protection, String> {
    let rules = metadata.protection.as_ref().ok_or_else(|| {
        metadata
            .protection_error
            .clone()
            .unwrap_or_else(|| "Protection metadata is unavailable".into())
    })?;
    let character = data["character"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("Invalid character")?;
    let character_bonus = *rules
        .character_bonus
        .get(&character)
        .ok_or("Character protection is unavailable")?;
    let run_bonus = setting(data, "bonuslimbarmor")?;
    let saved_limbs = data["limbs"].as_array().ok_or("Invalid limbs")?;
    if saved_limbs.len() != metadata.limbs.len() || saved_limbs.is_empty() {
        return Err("Saved limbs do not match the game".into());
    }
    let mut limbs = Vec::with_capacity(saved_limbs.len());
    for limb in saved_limbs {
        let prosthetic = match limb.get("prostheticInfo") {
            None | Some(Value::Null) => 0.0,
            Some(Value::Object(info)) => info
                .get("armorChange")
                .map_or(Some(0.0), Value::as_f64)
                .filter(|value| value.is_finite())
                .ok_or("Invalid prosthetic armor")? as f32,
            Some(_) => return Err("Invalid prosthetic armor".into()),
        };
        let divisor = rules.base + run_bonus + character_bonus + prosthetic;
        if !divisor.is_finite() {
            return Err("Invalid limb protection".into());
        }
        limbs.push(LimbProtection {
            divisor,
            clothing: Vec::new(),
            prosthetic,
        });
    }
    let items = data["items"].as_array().ok_or("Invalid inventory")?;
    if metadata.hold_slots.is_empty()
        || items.iter().any(|item| {
            item["slot"].as_i64().is_none_or(|slot| {
                slot < -1 || (slot >= 0 && slot as u64 >= metadata.hold_slots.len() as u64)
            })
        })
    {
        return Err("Invalid inventory slot prevents protection calculation".into());
    }
    let mut clothing_insulation = 0.0_f32;
    let mut occupied = HashSet::new();
    for group in group_items(data) {
        let Location::Worn(slot) = group.location else {
            continue;
        };
        let item = &items[group.indices[0]];
        let id = item["id"].as_str().ok_or("Invalid worn item ID")?;
        let definition = stats
            .get(id)
            .ok_or_else(|| format!("Unknown worn item: {id}"))?;
        if definition.destroy_at_zero_condition
            && item["condition"]
                .as_f64()
                .is_some_and(|condition| condition <= 0.0)
        {
            return Err(format!(
                "Worn item will be destroyed at zero condition: {id}"
            ));
        }
        if !definition.wearable
            || definition.wear_slot.as_deref() != Some(&slot)
            || !occupied.insert(slot)
        {
            return Err(format!("Invalid worn item placement: {id}"));
        }

        let primary = definition
            .desired_wear_limb
            .as_deref()
            .ok_or_else(|| format!("Missing wear limb for {id}"))?;
        let primary_index = metadata
            .limbs
            .iter()
            .position(|limb| limb.object == primary)
            .ok_or_else(|| format!("Unknown wear limb for {id}"))?;
        if saved_limbs[primary_index]["dismembered"].as_bool() != Some(false) {
            return Err(format!("Wear limb is missing or invalid for {id}"));
        }
        let armor = definition.armor.unwrap_or(rules.default_armor);
        let insulation = definition.insulation.unwrap_or(rules.default_insulation);
        if !armor.is_finite() || !insulation.is_finite() {
            return Err(format!("Invalid wearable protection for {id}"));
        }
        clothing_insulation += insulation;
        let secondary = rules
            .secondary_limbs
            .get(id)
            .ok_or_else(|| format!("Wearable coverage unavailable for {id}"))?;
        for limb in std::iter::once(primary).chain(secondary.iter().map(String::as_str)) {
            let index = metadata
                .limbs
                .iter()
                .position(|known| known.object == limb)
                .ok_or_else(|| format!("Unknown covered limb for {id}"))?;
            limbs[index].divisor += armor;
            limbs[index].clothing.push((id.to_owned(), armor));
        }
    }
    for limb in &mut limbs {
        if !limb.divisor.is_finite() {
            return Err("Invalid limb protection".into());
        }
        limb.divisor = limb.divisor.max(rules.minimum);
    }
    if !clothing_insulation.is_finite() {
        return Err("Invalid clothing insulation".into());
    }
    Ok(Protection {
        limbs,
        clothing_insulation,
        run_bonus,
        character_bonus,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{game_data::protection::ProtectionData, schema::LimbDef};
    use serde_json::json;

    fn wearable(slot: &str, primary: &str, armor: f32, insulation: f32) -> ItemStats {
        ItemStats {
            category: String::new(),
            weight: 1.0,
            trade_value: 0.0,
            untradeable: false,
            scale_weight_with_condition: false,
            wearable: true,
            wear_slot: Some(slot.into()),
            armor: Some(armor),
            insulation: Some(insulation),
            item_tags: HashSet::new(),
            container_capacity: None,
            container_max_item_weight: None,
            container_encumbrance: None,
            container_tag_restriction: None,
            liquid_capacity: None,
            only_hold_in_hands: false,
            wearable_can_be_held: false,
            desired_wear_limb: Some(primary.into()),
            destroy_at_zero_condition: false,
        }
    }

    fn fixture() -> (Value, HashMap<String, ItemStats>, Metadata) {
        let data = json!({
            "character": 1, "runSettings": [{"Item1":"bonuslimbarmor", "Item2":"0.5"}],
            "limbs": [{"dismembered":false,"prostheticInfo":{"armorChange":0.3}}, {"dismembered":false}],
            "items": [
                {"id":"coat","slot":-1,"wearSlot":"torso","condition":0.1},
                {"id":"stored","slot":-1,"wearSlot":"torso","condition":1.0},
                {"id":"helmet","slot":-1,"wearSlot":"hat","condition":1.0},
                {"id":"helmet","slot":0,"wearSlot":null,"condition":1.0}
            ]
        });
        let stats = [
            ("coat".into(), wearable("torso", "Torso", 0.25, 0.6)),
            ("helmet".into(), wearable("hat", "Head", 2.0, 0.2)),
            ("stored".into(), wearable("hat", "Head", 9.0, 9.0)),
        ]
        .into();
        let rules = ProtectionData {
            secondary_limbs: [
                ("coat".into(), vec!["Torso".into(), "Head".into()]),
                ("helmet".into(), vec![]),
            ]
            .into(),
            character_bonus: [(1, 0.2)].into(),
            base: 1.0,
            minimum: 0.1,
            default_armor: 0.0,
            default_insulation: 0.0,
            insulation: None,
        };
        let metadata = Metadata {
            limbs: vec![
                LimbDef {
                    object: "Torso",
                    name: "Torso",
                },
                LimbDef {
                    object: "Head",
                    name: "Head",
                },
            ],
            hold_slots: vec![crate::schema::HoldSlotDef {
                name: "Hand",
                locale_key: "",
                icon: "",
            }],
            protection: Some(rules),
            ..Default::default()
        };
        (data, stats, metadata)
    }

    #[test]
    fn armor_counts_primary_and_each_secondary_but_not_held_or_stored_items() {
        let (data, stats, metadata) = fixture();
        let before = data.clone();
        let result = calculate_protection(&data, &stats, &metadata).unwrap();
        assert!((result.limbs[0].divisor - 2.5).abs() < 0.00001);
        assert!((result.limbs[1].divisor - 3.95).abs() < 0.00001);
        assert_eq!(result.limbs[0].clothing.len(), 2);
        assert_eq!(result.limbs[1].clothing.len(), 2);
        assert!((result.clothing_insulation - 0.8).abs() < 0.00001);
        assert_eq!(data, before);
    }

    #[test]
    fn negative_bonuses_use_native_floor_and_saved_numeric_settings() {
        let (mut data, stats, metadata) = fixture();
        data["items"] = json!([]);
        data["runSettings"][0]["Item2"] = json!(-5.0);
        let result = calculate_protection(&data, &stats, &metadata).unwrap();
        assert_eq!(result.limbs[0].divisor, 0.1);
        assert_eq!(result.limbs[1].divisor, 0.1);
    }

    #[test]
    fn invalid_or_missing_inputs_do_not_produce_a_misleading_total() {
        let (mut data, stats, metadata) = fixture();
        data["runSettings"] = json!([]);
        assert!(calculate_protection(&data, &stats, &metadata).is_err());
        let (mut data, stats, metadata) = fixture();
        data["items"][0]["wearSlot"] = json!("hat");
        assert!(calculate_protection(&data, &stats, &metadata).is_err());
        let (mut data, stats, metadata) = fixture();
        data["limbs"][0]["dismembered"] = json!(true);
        assert!(calculate_protection(&data, &stats, &metadata).is_err());
        let (mut data, stats, metadata) = fixture();
        data["items"][0]["id"] = json!("unknown");
        assert!(calculate_protection(&data, &stats, &metadata).is_err());
    }
}
