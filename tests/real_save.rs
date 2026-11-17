//! Round-trip check against a real save, if one is available locally
//! (`Exemple/save.sv` is git-ignored, so the test is skipped otherwise).

use std::io::Read;
use std::path::Path;

use cu_save_editor::save::{self, SaveFormat};
use cu_save_editor::{game_data, model};

#[test]
fn example_inventory_can_be_balanced_with_each_rule() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Exemple")
        .join("save.sv");
    let (Ok(bytes), Some(game_dir)) = (std::fs::read(path), std::env::var_os("CU_GAME_DIR")) else {
        return;
    };
    let (original, _) = save::decode(&bytes).unwrap();
    let stats = game_data::load_item_stats(Path::new(&game_dir)).unwrap();
    let pairs = |data: &serde_json::Value| {
        let mut pairs: Vec<_> = data["items"]
            .as_array()
            .unwrap()
            .iter()
            .zip(data["itemComponents"].as_array().unwrap())
            .map(|(item, component)| {
                let mut item = item.clone();
                item.as_object_mut().unwrap().shift_remove("slot");
                item.as_object_mut().unwrap().shift_remove("wearSlot");
                serde_json::to_string(&(item, component)).unwrap()
            })
            .collect();
        pairs.sort();
        pairs
    };
    let mut failures = Vec::new();
    for rule in model::BalanceRule::ALL {
        let mut data = original.clone();
        let start = std::time::Instant::now();
        let result = model::balance_containers(&mut data, &stats, rule, true);
        eprintln!("{rule:?}: {result:?} in {:?}", start.elapsed());
        if let Err(error) = result {
            failures.push(format!("{rule:?}: {error}"));
        } else {
            assert_eq!(pairs(&data), pairs(&original));
            for group in model::group_items(&data) {
                if let Some(space) = model::container_space(&data, group.indices[0], &stats) {
                    assert_eq!(space.over_capacity, 0.0, "{rule:?}");
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn example_magazine_and_weapon_reload_conserve_ammunition() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Exemple")
        .join("save.sv");
    let (Ok(bytes), Some(game_dir)) = (std::fs::read(path), std::env::var_os("CU_GAME_DIR")) else {
        return;
    };
    game_data::load_game_data(Path::new(&game_dir)).unwrap();
    let (mut data, _) = save::decode(&bytes).unwrap();
    let total = |data: &serde_json::Value| {
        data["items"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(
                |(index, _)| match model::ammunition_status(data, index).unwrap() {
                    Some(model::AmmunitionStatus::Round) => 1,
                    Some(model::AmmunitionStatus::Magazine { rounds, .. }) => rounds,
                    Some(model::AmmunitionStatus::Weapon {
                        rounds, chamber, ..
                    }) => rounds + i64::from(chamber == model::ChamberState::Round),
                    None => 0,
                },
            )
            .sum::<i64>()
    };
    let before = total(&data);
    for id in ["smallmagazine", "pistol"] {
        let index = data["items"]
            .as_array()
            .unwrap()
            .iter()
            .position(|item| item["id"] == id)
            .unwrap();
        let index = model::reload_ammunition(&mut data, index).unwrap();
        assert_eq!(data["items"][index]["id"], id);
        assert_eq!(total(&data), before);
        assert_eq!(
            data["items"].as_array().unwrap().len(),
            data["itemComponents"].as_array().unwrap().len()
        );
        match model::ammunition_status(&data, index).unwrap().unwrap() {
            model::AmmunitionStatus::Magazine { rounds, capacity } => assert_eq!(rounds, capacity),
            model::AmmunitionStatus::Weapon {
                rounds,
                capacity,
                chamber,
                ..
            } => {
                assert_eq!(rounds, capacity);
                assert_eq!(chamber, model::ChamberState::Round);
            }
            _ => panic!("reloaded item is not a magazine or weapon"),
        }
    }
}

#[test]
fn real_save_round_trips_to_identical_json() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Exemple")
        .join("save.sv");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipped: {} not found", path.display());
        return;
    };
    let mut original = String::new();
    flate2::read::GzDecoder::new(&bytes[..])
        .read_to_string(&mut original)
        .unwrap();

    let (data, format) = save::decode(&bytes).unwrap();
    assert_eq!(format, SaveFormat::Gzip);
    let encoded = save::encode(&data, format);
    let mut text = String::new();
    flate2::read::GzDecoder::new(&encoded[..])
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(
        text, original,
        "re-encoded JSON differs from the game's output"
    );
}
