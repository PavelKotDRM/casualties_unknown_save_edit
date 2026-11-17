//! Atomic ammunition transfers; reloads never manufacture rounds or magazines.

use std::collections::HashSet;

use serde_json::{Map, Value, json};

use crate::game_data::{ammunition::AmmunitionData, runtime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChamberState {
    Round,
    Casing,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmunitionStatus {
    Round,
    Magazine {
        rounds: i64,
        capacity: i64,
    },
    Weapon {
        rounds: i64,
        capacity: i64,
        magazine: bool,
        chamber: ChamberState,
    },
}

fn component(
    data: &Value,
    index: usize,
    name: &str,
    defaults: &Map<String, Value>,
) -> Result<(String, Map<String, Value>), String> {
    let saved = data
        .get("itemComponents")
        .and_then(|v| v.get(index))
        .and_then(Value::as_object)
        .ok_or("Missing or invalid item component data")?;
    let mut matching = saved
        .iter()
        .filter(|(key, _)| key.split(',').next() == Some(name));
    let mut values = defaults.clone();
    let key = if let Some((key, saved)) = matching.next() {
        if matching.next().is_some() {
            return Err(format!("Duplicate {name} components"));
        }
        values.extend(
            saved
                .as_object()
                .ok_or_else(|| format!("Invalid {name} state"))?
                .clone(),
        );
        key.clone()
    } else {
        format!("{name}, Assembly-CSharp")
    };
    Ok((key, values))
}

fn write_component(data: &mut Value, index: usize, key: String, state: Map<String, Value>) {
    data["itemComponents"][index]
        .as_object_mut()
        .unwrap()
        .insert(key, state.into());
}

fn integer(state: &Map<String, Value>, key: &str) -> Result<i64, String> {
    state
        .get(key)
        .and_then(Value::as_i64)
        .filter(|v| (0..=i64::from(i32::MAX)).contains(v))
        .ok_or_else(|| format!("Missing or invalid ammunition field {key}"))
}

fn count(state: &Map<String, Value>, rounds: &str, capacity: &str) -> Result<(i64, i64), String> {
    let loaded = integer(state, rounds)?;
    let capacity = integer(state, capacity)?;
    if loaded > capacity || capacity == 0 {
        return Err(format!(
            "Invalid ammunition count or capacity: {loaded}/{capacity}"
        ));
    }
    Ok((loaded, capacity))
}

fn flag(state: &Map<String, Value>, key: &str) -> Result<bool, String> {
    state
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("Missing or invalid weapon field {key}"))
}

fn chamber(
    state: &Map<String, Value>,
    definitions: &AmmunitionData,
) -> Result<ChamberState, String> {
    match state.get("roundInChamber").and_then(Value::as_i64) {
        Some(v) if v == definitions.chamber.round => Ok(ChamberState::Round),
        Some(v) if v == definitions.chamber.casing => Ok(ChamberState::Casing),
        Some(v) if v == definitions.chamber.empty => Ok(ChamberState::Empty),
        _ => Err("Invalid weapon chamber state".into()),
    }
}

fn item_id(data: &Value, index: usize) -> Result<&str, String> {
    data.get("items")
        .and_then(|v| v.get(index))
        .and_then(|v| v.get("id"))
        .and_then(Value::as_str)
        .ok_or("Missing ammunition item ID".into())
}

pub fn ammunition_status(data: &Value, index: usize) -> Result<Option<AmmunitionStatus>, String> {
    let id = item_id(data, index)?;
    let metadata = runtime::metadata();
    let Some(definitions) = &metadata.ammunition else {
        let is_ammunition = data
            .get("itemComponents")
            .and_then(|v| v.get(index))
            .and_then(Value::as_object)
            .is_some_and(|components| {
                components
                    .keys()
                    .any(|key| matches!(key.split(',').next(), Some("AmmoScript" | "GunScript")))
            });
        return if is_ammunition {
            Err(metadata
                .ammunition_error
                .clone()
                .unwrap_or_else(|| "Installed-game ammunition metadata is unavailable".into()))
        } else {
            Ok(None)
        };
    };
    if let Some(def) = definitions.items.get(id) {
        if !def.magazine {
            return Ok(Some(AmmunitionStatus::Round));
        }
        let (_, state) = component(data, index, "AmmoScript", &def.defaults)?;
        let (rounds, capacity) = count(&state, "rounds", "maxRounds")?;
        Ok(Some(AmmunitionStatus::Magazine { rounds, capacity }))
    } else if let Some(def) = definitions.guns.get(id) {
        let (_, state) = component(data, index, "GunScript", &def.defaults)?;
        let (rounds, capacity) = count(&state, "roundsInMag", "magCapacity")?;
        let magazine = flag(&state, "hasMag")?;
        if def.magazine_fed && !magazine && rounds > 0 {
            return Err("Weapon has ammunition in a missing magazine".into());
        }
        Ok(Some(AmmunitionStatus::Weapon {
            rounds,
            capacity,
            magazine: magazine || !def.magazine_fed,
            chamber: chamber(&state, definitions)?,
        }))
    } else {
        Ok(None)
    }
}

struct Supply {
    index: usize,
    key: String,
    state: Map<String, Value>,
    magazine: bool,
    insertable: bool,
    rounds: i64,
    capacity: i64,
}

fn supplies(
    data: &Value,
    target: usize,
    ammo_type: i64,
    definitions: &AmmunitionData,
) -> Result<Vec<Supply>, String> {
    let holders: HashSet<_> = super::group_items(data)
        .into_iter()
        .filter(|group| group.indices.len() > 1)
        .map(|group| group.indices[0])
        .collect();
    let mut supplies = Vec::new();
    for (index, item) in data["items"].as_array().unwrap().iter().enumerate() {
        if index == target
            || holders.contains(&index)
            || item.get("favourited").and_then(Value::as_bool) == Some(true)
        {
            continue;
        }
        let Some(def) = item
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| definitions.items.get(id))
            .filter(|def| def.ammo_type == ammo_type)
        else {
            continue;
        };
        if !item
            .get("condition")
            .and_then(Value::as_f64)
            .is_some_and(|v| v.is_finite() && v > 0.0)
        {
            return Err(format!("Invalid ammunition condition for item #{index}"));
        }
        let (key, state) = component(data, index, "AmmoScript", &def.defaults)?;
        let (rounds, capacity) = if def.magazine {
            count(&state, "rounds", "maxRounds")?
        } else {
            (1, 1)
        };
        supplies.push(Supply {
            index,
            key,
            state,
            magazine: def.magazine,
            insertable: def.insertable,
            rounds,
            capacity,
        });
    }
    supplies.sort_by_key(|supply| supply.magazine);
    Ok(supplies)
}

fn take_rounds(supplies: &mut [Supply], removed: &HashSet<usize>, requested: i64) -> i64 {
    let mut remaining = requested;
    for supply in supplies.iter_mut().filter(|s| !removed.contains(&s.index)) {
        let taken = remaining.min(supply.rounds);
        supply.rounds -= taken;
        remaining -= taken;
        if remaining == 0 {
            break;
        }
    }
    requested - remaining
}

/// Reloads from compatible non-favourite inventory ammunition. Empty boxes and
/// magazines remain in place; loose rounds and inserted magazines are consumed.
/// A weapon is chambered and closed, with its safety setting preserved.
/// Returns the target's new index after removing consumed items.
pub fn reload_ammunition(data: &mut Value, target: usize) -> Result<usize, String> {
    let items = data
        .get("items")
        .and_then(Value::as_array)
        .ok_or("Missing items array")?;
    let components = data
        .get("itemComponents")
        .and_then(Value::as_array)
        .ok_or("Missing itemComponents array")?;
    if items.len() != components.len() || components.iter().any(|value| !value.is_object()) {
        return Err("Items and component arrays must be valid and aligned before reloading".into());
    }
    let metadata = runtime::metadata();
    let definitions = metadata.ammunition.as_ref().ok_or_else(|| {
        metadata
            .ammunition_error
            .clone()
            .unwrap_or_else(|| "Installed-game ammunition metadata is unavailable".into())
    })?;
    let id = item_id(data, target)?;
    let status = ammunition_status(data, target)?.ok_or("This item cannot be reloaded")?;
    let (ammo_type, name, defaults) = if let Some(def) = definitions.guns.get(id) {
        (def.ammo_type, "GunScript", &def.defaults)
    } else {
        let def = definitions
            .items
            .get(id)
            .ok_or("Missing ammunition definition")?;
        if !def.magazine {
            return Err("Loose rounds cannot be reloaded; select a magazine or weapon".into());
        }
        (def.ammo_type, "AmmoScript", &def.defaults)
    };
    let (key, mut state) = component(data, target, name, defaults)?;
    let mut supplies = supplies(data, target, ammo_type, definitions)?;
    let mut removed = HashSet::new();
    let mut changed = false;
    match status {
        AmmunitionStatus::Magazine { rounds, capacity } => {
            let taken = take_rounds(&mut supplies, &removed, capacity - rounds);
            if taken > 0 {
                state.insert("rounds".into(), json!(rounds + taken));
                changed = true;
            }
        }
        AmmunitionStatus::Weapon {
            mut rounds,
            capacity,
            mut magazine,
            chamber,
        } => {
            if !magazine
                && let Some(source) = supplies
                    .iter()
                    .filter(|s| s.magazine && s.insertable && s.capacity <= capacity)
                    .max_by_key(|s| s.rounds)
            {
                removed.insert(source.index);
                magazine = true;
                rounds = source.rounds;
                state.insert("hasMag".into(), json!(true));
                changed = true;
            }
            if chamber != ChamberState::Round {
                let chambered = if magazine && rounds > 0 {
                    rounds -= 1;
                    true
                } else {
                    take_rounds(&mut supplies, &removed, 1) == 1
                };
                if chambered {
                    state.insert("roundInChamber".into(), json!(definitions.chamber.round));
                    changed = true;
                }
            }
            if magazine {
                let taken = take_rounds(&mut supplies, &removed, capacity - rounds);
                rounds += taken;
                changed |= taken > 0;
            }
            if changed {
                if chamber == ChamberState::Casing
                    && state.get("roundInChamber").and_then(Value::as_i64)
                        == Some(definitions.chamber.casing)
                {
                    state.insert("roundInChamber".into(), json!(definitions.chamber.empty));
                }
                state.insert("roundsInMag".into(), json!(rounds));
                for field in ["triggerPressed", "firingPinStruck", "racked", "lastRacked"] {
                    state.insert(field.into(), json!(false));
                }
            }
        }
        AmmunitionStatus::Round => return Err("Loose rounds cannot be reloaded".into()),
    }
    if !changed {
        return Err(
            "Already loaded, or no compatible non-favourite ammunition is available".into(),
        );
    }
    let mut updated = data.clone();
    write_component(&mut updated, target, key, state);
    for supply in supplies {
        if removed.contains(&supply.index) {
            continue;
        }
        if supply.magazine
            && supply.state.get("rounds").and_then(Value::as_i64) != Some(supply.rounds)
        {
            let mut state = supply.state;
            state.insert("rounds".into(), json!(supply.rounds));
            write_component(&mut updated, supply.index, supply.key, state);
        } else if !supply.magazine && supply.rounds == 0 {
            removed.insert(supply.index);
        }
    }
    let new_index = target - removed.iter().filter(|&&index| index < target).count();
    let mut removed: Vec<_> = removed.into_iter().collect();
    removed.sort_unstable_by(|a, b| b.cmp(a));
    for index in removed {
        super::remove_item(&mut updated, index).ok_or("Cannot remove consumed ammunition")?;
    }
    *data = updated;
    Ok(new_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_data::{
        ammunition::{AmmoDef, ChamberValues, GunDef},
        metadata::Metadata,
    };

    fn fixture() -> runtime::TestMetadata {
        let state = |value: Value| value.as_object().unwrap().clone();
        let ammo = |ammo_type, magazine, capacity, rounds| AmmoDef {
            ammo_type,
            magazine,
            insertable: magazine && capacity == 3,
            defaults: state(json!({"maxRounds": capacity, "rounds": rounds})),
        };
        let gun = |magazine_fed| GunDef {
            ammo_type: 7,
            magazine_fed,
            defaults: state(
                json!({"roundInChamber": 30, "roundsInMag": 0, "magCapacity": 4,
                "hasMag": false, "triggerPressed": false, "firingPinStruck": false,
                "safe": true, "racked": false, "lastRacked": false}),
            ),
        };
        runtime::scoped_for_test(Metadata {
            ammunition: Some(AmmunitionData {
                items: [
                    ("round".into(), ammo(7, false, 0, 0)),
                    ("other-round".into(), ammo(8, false, 0, 0)),
                    ("magazine".into(), ammo(7, true, 3, 0)),
                    ("box".into(), ammo(7, true, 6, 5)),
                ]
                .into(),
                guns: [("gun".into(), gun(true)), ("direct".into(), gun(false))].into(),
                chamber: ChamberValues {
                    round: 10,
                    casing: 20,
                    empty: 30,
                },
            }),
            ..Default::default()
        })
    }

    fn document(ids: &[&str]) -> Value {
        json!({
            "items": ids.iter().enumerate().map(|(i,id)| json!({
                "id": id, "condition": 1.0, "slot": i, "label": format!("item-{i}")
            })).collect::<Vec<_>>(),
            "itemComponents": ids.iter().enumerate().map(|(i,_)| json!({"Other": {"marker": i}})).collect::<Vec<_>>()
        })
    }

    fn total(data: &Value) -> i64 {
        data["items"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(
                |(index, _)| match ammunition_status(data, index).unwrap().unwrap() {
                    AmmunitionStatus::Round => 1,
                    AmmunitionStatus::Magazine { rounds, .. } => rounds,
                    AmmunitionStatus::Weapon {
                        rounds, chamber, ..
                    } => rounds + i64::from(chamber == ChamberState::Round),
                },
            )
            .sum()
    }

    #[test]
    fn magazine_reload_consumes_loose_rounds_and_keeps_components_aligned() {
        let _metadata = fixture();
        let mut data = document(&["round", "round", "magazine"]);
        let before = total(&data);
        assert_eq!(reload_ammunition(&mut data, 2), Ok(0));
        assert_eq!(total(&data), before);
        assert_eq!(data["items"].as_array().unwrap().len(), 1);
        assert_eq!(data["itemComponents"][0]["Other"]["marker"], 2);
        assert_eq!(
            ammunition_status(&data, 0).unwrap(),
            Some(AmmunitionStatus::Magazine {
                rounds: 2,
                capacity: 3
            })
        );
    }

    #[test]
    fn magazine_reload_transfers_from_boxes_and_preserves_unrelated_state() {
        let _metadata = fixture();
        let mut data = document(&["magazine", "box", "other-round", "round", "magazine"]);
        data["items"][3]["favourited"] = json!(true);
        data["itemComponents"][1]["AmmoScript"] = json!({"rounds": 5, "custom": "keep"});
        let before = total(&data);
        let untouched = data["itemComponents"][4].clone();
        reload_ammunition(&mut data, 0).unwrap();
        assert_eq!(total(&data), before);
        assert_eq!(data["itemComponents"][1]["AmmoScript"]["rounds"], 2);
        assert_eq!(data["itemComponents"][1]["AmmoScript"]["custom"], "keep");
        assert_eq!(data["itemComponents"][4], untouched);
        assert_eq!(data["items"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn weapon_reload_inserts_a_magazine_chambers_and_preserves_safety() {
        let _metadata = fixture();
        let mut data = document(&["magazine", "round", "round", "gun"]);
        data["itemComponents"][0]["AmmoScript"] = json!({"rounds": 3});
        data["itemComponents"][3]["GunScript"] = json!({
            "roundInChamber": 20, "racked": true, "lastRacked": true,
            "triggerPressed": true, "firingPinStruck": true, "custom": 123
        });
        let before = total(&data);
        let index = reload_ammunition(&mut data, 3).unwrap();
        assert_eq!(total(&data), before);
        assert_eq!(index, 0);
        assert_eq!(data["items"][index]["id"], "gun");
        let gun = &data["itemComponents"][index]["GunScript"];
        assert_eq!(gun["roundsInMag"], 4);
        assert_eq!(gun["roundInChamber"], 10);
        assert_eq!(gun["hasMag"], true);
        assert_eq!(gun["safe"], true);
        assert_eq!(gun["racked"], false);
        assert_eq!(gun["lastRacked"], false);
        assert_eq!(gun["triggerPressed"], false);
        assert_eq!(gun["firingPinStruck"], false);
        assert_eq!(gun["custom"], 123);
        assert_eq!(data["itemComponents"][index]["Other"]["marker"], 3);
    }

    #[test]
    fn direct_feed_reload_keeps_the_emptied_box_and_needs_no_magazine() {
        let _metadata = fixture();
        let mut data = document(&["direct", "box"]);
        let before = total(&data);
        reload_ammunition(&mut data, 0).unwrap();
        assert_eq!(total(&data), before);
        assert_eq!(data["items"].as_array().unwrap().len(), 2);
        assert_eq!(
            data["itemComponents"][1]["AmmoScript, Assembly-CSharp"]["rounds"],
            0
        );
        assert_eq!(
            ammunition_status(&data, 0).unwrap(),
            Some(AmmunitionStatus::Weapon {
                rounds: 4,
                capacity: 4,
                magazine: true,
                chamber: ChamberState::Round
            })
        );
    }

    #[test]
    fn ammunition_boxes_are_not_inserted_as_weapon_magazines() {
        let _metadata = fixture();
        let mut data = document(&["gun", "box"]);
        data["itemComponents"][1]["AmmoScript"] = json!({"rounds":2, "maxRounds":2});
        let before = total(&data);
        reload_ammunition(&mut data, 0).unwrap();
        assert_eq!(total(&data), before);
        assert_eq!(data["items"].as_array().unwrap().len(), 2);
        assert_eq!(data["itemComponents"][1]["AmmoScript"]["rounds"], 1);
        assert_eq!(
            ammunition_status(&data, 0).unwrap(),
            Some(AmmunitionStatus::Weapon {
                rounds: 0,
                capacity: 4,
                magazine: false,
                chamber: ChamberState::Round
            })
        );
    }

    #[test]
    fn reload_failures_do_not_mutate_the_inventory() {
        let _metadata = fixture();
        let base = document(&["magazine", "round"]);
        let mut favourite = base.clone();
        favourite["items"][1]["favourited"] = json!(true);
        let wrong_type = document(&["magazine", "other-round"]);
        let mut bad_capacity = base.clone();
        bad_capacity["itemComponents"][0]["AmmoScript"] = json!({"maxRounds": 0});
        let mut full = base.clone();
        full["itemComponents"][0]["AmmoScript"] = json!({"rounds": 3});
        let mut unaligned = base.clone();
        unaligned["itemComponents"] = json!([]);
        for mut data in [favourite, wrong_type, bad_capacity, full, unaligned] {
            let before = data.clone();
            assert!(reload_ammunition(&mut data, 0).is_err());
            assert_eq!(data, before);
        }
    }
}
