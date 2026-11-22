//! Optional integration check against a locally installed game.

use std::path::Path;

use cu_save_editor::game_info::GameInfo;
use cu_save_editor::schema;

#[test]
fn russian_interface_translates_installed_health_and_limb_fields() {
    let Some(game_dir) = std::env::var_os("CU_GAME_DIR") else {
        eprintln!("skipped: CU_GAME_DIR is not set");
        return;
    };
    let game = cu_save_editor::game_data::load_game_data(Path::new(&game_dir)).unwrap();
    let lang_dir = Path::new(&game_dir)
        .join("CasualtiesUnknown_Data")
        .join("Lang");
    let english = cu_save_editor::locale::Locale::load(&lang_dir.join("EN.json")).unwrap();
    let russian = cu_save_editor::locale::Locale::load(&lang_dir.join("ru-RU.json")).unwrap();
    for locale in [None, Some(&english), Some(&russian)] {
        let names = cu_save_editor::ui::Names {
            locale,
            interface_language: cu_save_editor::ui::InterfaceLanguage::Russian,
        };
        let missing: Vec<_> = game
            .metadata
            .body_groups
            .iter()
            .flat_map(|group| group.fields.iter())
            .chain(game.metadata.limb_fields.iter())
            .filter_map(|field| {
                let label = names.field(field);
                (!label
                    .chars()
                    .any(|c| ('А'..='я').contains(&c) || c == 'ё' || c == 'Ё'))
                .then(|| format!("{}: {label}", field.key))
            })
            .collect();
        assert!(
            missing.is_empty(),
            "Untranslated health fields: {missing:?}"
        );
    }

    let names = cu_save_editor::ui::Names {
        locale: None,
        interface_language: cu_save_editor::ui::InterfaceLanguage::Russian,
    };
    for source in &game.drop_sources {
        let label = names.game_identifier(&source.name);
        assert!(
            label
                .chars()
                .any(|c| ('А'..='я').contains(&c) || c == 'ё' || c == 'Ё'),
            "Untranslated drop source {}: {label}",
            source.name
        );
    }
    for slot in game
        .items
        .values()
        .filter_map(|item| item.wear_slot.as_deref())
    {
        let label = names.wear_slot(slot);
        assert!(
            label
                .chars()
                .any(|c| ('А'..='я').contains(&c) || c == 'ё' || c == 'Ё'),
            "Untranslated wear slot {slot}: {label}"
        );
    }
}

#[test]
fn runtime_catalogues_and_rules_come_from_the_installed_game() {
    let Some(game_dir) = std::env::var_os("CU_GAME_DIR") else {
        eprintln!("skipped: CU_GAME_DIR is not set");
        return;
    };
    let data = cu_save_editor::game_data::load_game_data(Path::new(&game_dir)).unwrap();
    assert_eq!(data.crafting.len(), 137);
    let rope_icon = data.crafting_icons.get("rope").unwrap();
    assert!(rope_icon.width > 0 && rope_icon.height > 0);
    assert_eq!(
        rope_icon.rgba.len(),
        rope_icon.width as usize * rope_icon.height as usize * 4
    );
    let item_icon = data.crafting_icons.get("scrapmetal").unwrap();
    assert!(item_icon.width > 0 && item_icon.height > 0);
    assert!(!data.drop_sources.is_empty());
    assert!(data.drop_sources.iter().any(|source| {
        source.name == "medcrate" && (!source.drops.is_empty() || !source.always_drops.is_empty())
    }));
    let rope = data
        .crafting
        .iter()
        .find(|recipe| recipe.index == 0)
        .unwrap();
    assert_eq!(rope.result_id, "rope");
    assert_eq!(rope.intelligence, 1);
    assert_eq!(rope.ingredients.len(), 2);
    assert_eq!(rope.ingredients[0].quality_id.as_deref(), Some("foliage"));
    assert_eq!(rope.ingredients[0].minimum_condition, 0.0);
    assert_eq!(rope.ingredients[0].ignored_id.as_deref(), Some("rope"));
    let metadata = &data.metadata;
    let protection = metadata.protection.as_ref().unwrap_or_else(|| {
        panic!(
            "Protection metadata unavailable: {:?}",
            metadata.protection_error
        )
    });
    assert_eq!(protection.base, 1.0);
    assert_eq!(protection.minimum, 0.1);
    assert!(
        protection
            .secondary_limbs
            .values()
            .any(|limbs| !limbs.is_empty())
    );
    assert_eq!(protection.character_bonus.len(), metadata.characters.len());
    assert_eq!(protection.character_bonus[&2], 0.2);
    assert!(protection.secondary_limbs["hoodie"].contains(&"UpTorso".to_owned()));
    assert!(protection.secondary_limbs["hoodie"].contains(&"HandF".to_owned()));
    assert!(protection.secondary_limbs["riothelmet"].is_empty());
    let limbs: Vec<_> = metadata
        .limbs
        .iter()
        .map(|_| serde_json::json!({"dismembered": false}))
        .collect();
    let save = serde_json::json!({
        "character": 2, "body": {"weightOffset": 10.0},
        "runSettings": [
            {"Item1": "bonuslimbarmor", "Item2": "0.5"},
            {"Item1": "insulationmultiplier", "Item2": "1.25"}
        ],
        "limbs": limbs, "items": [
            {"id": "hoodie", "slot": -1, "wearSlot": "outertorso", "condition": 0.2},
            {"id": "riothelmet", "slot": -1, "wearSlot": "hat", "condition": 0.9}
        ]
    });
    let calculated =
        cu_save_editor::model::calculate_protection(&save, &data.items, metadata).unwrap();
    let index = |name| {
        metadata
            .limbs
            .iter()
            .position(|limb| limb.object == name)
            .unwrap()
    };
    assert!((calculated.limbs[index("Head")].divisor - 3.7).abs() < 0.0001);
    assert!((calculated.limbs[index("UpTorso")].divisor - 2.2).abs() < 0.0001);
    assert!((calculated.limbs[index("HandF")].divisor - 1.95).abs() < 0.0001);
    assert!((calculated.clothing_insulation - 0.8).abs() < 0.0001);
    let insulation = cu_save_editor::model::calculate_insulation(
        &save,
        calculated.clothing_insulation,
        metadata,
    )
    .unwrap();
    assert!((insulation[0] - 1.75).abs() < 0.0001);
    assert!((insulation[1] - 3.0).abs() < 0.0001);
    let mut cold_save = save.clone();
    cold_save["character"] = serde_json::json!(0);
    cold_save["body"]["weightOffset"] = serde_json::json!(-80.0);
    cold_save["runSettings"][1]["Item2"] = serde_json::json!("0.5");
    let insulation =
        cu_save_editor::model::calculate_insulation(&cold_save, 0.0, metadata).unwrap();
    assert_eq!(insulation[0], 0.5, "native minimum insulation");
    assert!((insulation[1] - 0.6).abs() < 0.0001);
    cold_save["runSettings"][1]["Item2"] = serde_json::json!("invalid");
    assert!(cu_save_editor::model::calculate_insulation(&cold_save, 0.0, metadata).is_err());
    let assembly = dotscope::CilObject::from_path_with_validation(
        Path::new(&game_dir)
            .join("CasualtiesUnknown_Data")
            .join("Managed")
            .join("Assembly-CSharp.dll"),
        dotscope::ValidationConfig::minimal(),
    )
    .unwrap();
    let setup = assembly
        .methods()
        .iter()
        .find_map(|entry| {
            let method = entry.value();
            let owner = method.declaring_type.get()?.upgrade()?;
            (owner.name == "Item" && method.name == "SetupItems").then(|| method.clone())
        })
        .unwrap();
    let instructions: Vec<_> = setup.instructions().collect();
    let native_ids: std::collections::BTreeSet<_> = instructions
        .windows(2)
        .filter_map(|pair| {
            if pair[0].mnemonic != "ldstr" || pair[1].mnemonic != "newobj" {
                return None;
            }
            let dotscope::assembly::Operand::Token(constructor) = pair[1].operand else {
                return None;
            };
            if !assembly
                .methods()
                .get(&constructor)?
                .value()
                .signature
                .params
                .is_empty()
            {
                return None;
            }
            let dotscope::assembly::Operand::Token(token) = pair[0].operand else {
                return None;
            };
            assembly
                .userstrings()?
                .get((token.value() & 0x00ff_ffff) as usize)
                .ok()
                .map(|value| value.to_string_lossy())
        })
        .collect();
    assert_eq!(native_ids.len(), 300, "native item registry size");
    assert_eq!(
        data.items
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        native_ids
    );
    assert_eq!(
        data.effects
            .items
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        native_ids
    );
    for (id, weight, value, charge) in [
        ("smallbattery", 0.1, 6.0, 50.0),
        ("mediumbattery", 0.2, 12.0, 100.0),
        ("largebattery", 0.5, 20.0, 300.0),
    ] {
        assert_eq!(data.items[id].weight, weight, "weight of {id}");
        assert!(data.items[id].item_tags.contains("battery"));
        assert_eq!(data.effects.items[id].value, Some(value), "value of {id}");
        assert_eq!(
            data.effects.items[id].max_charge,
            Some(charge),
            "charge of {id}"
        );
        assert_eq!(metadata.battery_charges[id], charge);
        let item =
            cu_save_editor::model::new_item(id, &cu_save_editor::model::Location::Slot(0)).unwrap();
        assert_eq!(item["condition"], 1.0, "new battery charge");
        assert!(metadata.items.iter().any(|item| item.id == id));
    }
    assert_eq!(data.items["plushie"].weight, 0.15);
    assert!(data.items["plushie"].destroy_at_zero_condition);
    assert!(!data.items["plushie"].item_tags.contains("battery"));
    assert_eq!(data.effects.items["plushie"].value, Some(5.0));
    assert!(
        metadata.item_components_error.is_none(),
        "{:?}",
        metadata.item_components_error
    );
    let locale = cu_save_editor::locale::Locale::load(
        &Path::new(&game_dir)
            .join("CasualtiesUnknown_Data")
            .join("Lang")
            .join("EN.json"),
    );
    let names = cu_save_editor::ui::Names {
        locale: locale.as_ref(),
        interface_language: cu_save_editor::ui::InterfaceLanguage::English,
    };
    assert_eq!(
        (0..6).map(|slot| names.hold_slot(slot)).collect::<Vec<_>>(),
        [
            "Right hand",
            "Left hand",
            "Mouth",
            "Upper back",
            "Middle back",
            "Lower back"
        ]
    );
    let water = cu_save_editor::game_data::component_key("WaterContainerItem");
    for (id, stats) in &data.items {
        if let Some(capacity) = stats.liquid_capacity {
            let components = cu_save_editor::model::new_item_components(id).unwrap();
            let expected: Vec<_> = data.effects.items[id]
                .default_contents
                .iter()
                .map(|(liquid, amount)| serde_json::json!({"liquidId": liquid, "amount": amount}))
                .collect();
            assert_eq!(
                components[&water]["stack"],
                serde_json::json!(expected),
                "contents of {id}"
            );
            let item =
                cu_save_editor::model::new_item(id, &cu_save_editor::model::Location::Slot(0))
                    .unwrap();
            let total: f64 = expected
                .iter()
                .map(|entry| entry["amount"].as_f64().unwrap())
                .sum();
            assert!(
                (item["condition"].as_f64().unwrap() - total / f64::from(capacity)).abs() < 0.0001,
                "condition of {id}"
            );
        }
    }
    let battery = cu_save_editor::game_data::component_key("BatteryItem");
    let flashlight = cu_save_editor::model::new_item_components("flashlight").unwrap();
    assert_eq!(flashlight[&battery]["batteryType"], "mediumbattery");
    assert_eq!(flashlight[&battery]["maxCharge"], 100.0);
    assert_eq!(flashlight[&battery]["batteryWasFavourited"], false);
    assert_eq!(metadata.battery_limits["flashlight"], 100.0);
    assert_eq!(
        cu_save_editor::model::battery_capacity("flashlight", "smallbattery", metadata).unwrap(),
        50.0
    );
    assert!(
        cu_save_editor::model::battery_capacity("flashlight", "largebattery", metadata).is_err()
    );
    for (id, components) in &metadata.item_components {
        if let Some(component) = components.get(&battery) {
            assert!(
                metadata.battery_limits.contains_key(id),
                "battery limit of {id}"
            );
            match component["batteryType"]
                .as_str()
                .filter(|id| !id.is_empty())
            {
                Some("smallbattery") => assert_eq!(component["maxCharge"], 50.0, "{id}"),
                Some("mediumbattery") => assert_eq!(component["maxCharge"], 100.0, "{id}"),
                Some("largebattery") => assert_eq!(component["maxCharge"], 300.0, "{id}"),
                None => assert_eq!(component["maxCharge"], 0.0, "{id}"),
                Some(other) => panic!("Unexpected default battery {other} in {id}"),
            }
        }
    }
    let bread = metadata.condition_uses["bread"][0];
    assert_eq!(bread.cost, 0.34);
    assert_eq!(
        cu_save_editor::model::remaining_uses(1.0, bread).unwrap(),
        3
    );
    assert!(
        !metadata.condition_uses.contains_key("bandage"),
        "minigames are not fixed uses"
    );
    assert!(
        !metadata.condition_uses.contains_key("tourniquet"),
        "transferred items are not repeatable"
    );
    assert!(
        metadata
            .condition_uses
            .values()
            .flatten()
            .any(|rule| rule.minimum == 0.5 && rule.inclusive)
    );
    assert!(
        metadata
            .condition_uses
            .values()
            .flatten()
            .any(|rule| rule.minimum == 0.1 && !rule.inclusive)
    );
    assert!(
        metadata.moodles_error.is_none(),
        "{:?}",
        metadata.moodles_error
    );
    let [red, green, blue] = metadata.moodles.colours["pain2"];
    assert!(red > green && green >= blue);
    let [red, green, blue] = metadata.moodles.colours["energized"];
    assert!(green > red && green > blue);
    let [red, green, blue] = metadata.moodles.colours["focused"];
    assert!(blue > red && blue > green);
    assert_eq!(
        metadata.moodles.directions["Body.stamina"],
        schema::Good::High
    );
    assert_eq!(
        metadata.moodles.directions["Body.sicknessAmount"],
        schema::Good::Low
    );
    assert_eq!(
        metadata.moodles.directions["Body.heartRate"],
        schema::Good::Neutral
    );
    assert_eq!(
        metadata.moodles.directions["Body.focusedLevel"],
        schema::Good::Neutral
    );
    let ammunition = metadata.ammunition.as_ref().unwrap_or_else(|| {
        panic!(
            "ammunition metadata unavailable: {:?}",
            metadata.ammunition_error
        )
    });
    assert!(!ammunition.items["9mmround"].magazine);
    assert!(ammunition.items["smallmagazine"].magazine);
    assert!(ammunition.items["smallmagazine"].insertable);
    assert!(!ammunition.items["boxof12gauge"].insertable);
    assert_eq!(ammunition.items["boxof12gauge"].defaults["rounds"], 0);
    assert_eq!(ammunition.items["boxof12gauge"].defaults["maxRounds"], 16);
    assert_eq!(ammunition.items["smallmagazine"].defaults["maxRounds"], 17);
    assert_eq!(ammunition.guns["pistol"].defaults["magCapacity"], 17);
    assert_eq!(
        ammunition.guns["pistol"].defaults["roundInChamber"],
        ammunition.chamber.empty
    );
    assert_eq!(metadata.items.len(), data.items.len());
    assert_eq!(metadata.liquids.len(), data.effects.liquids.len());
    assert!(!metadata.characters.is_empty());
    assert!(!metadata.limbs.is_empty());
    assert!(!metadata.hold_slots.is_empty());
    assert!(!metadata.endings.is_empty());
    assert!(
        metadata
            .endings
            .iter()
            .all(|ending| !ending.status_key.is_empty())
    );
    assert!(metadata.items.iter().all(|item| {
        let stats = &data.items[item.id];
        item.category == stats.category && item.capacity == stats.liquid_capacity.unwrap_or(0.0)
    }));
    for (id, group) in [
        ("pistol", "weapons"),
        ("smallmagazine", "weapons"),
        ("shovel", "tools"),
        ("hoodie", "clothing"),
        ("tacticalboots", "clothing"),
        ("slingbag", "containers"),
        ("flashlight", "devices"),
        ("aquapple", "food"),
        ("canteen", "drinks"),
    ] {
        let item = metadata.items.iter().find(|item| item.id == id).unwrap();
        assert_eq!(item.group, group, "group of {id}");
    }
    assert!(!metadata.body_defaults.is_empty());
    assert!(!metadata.limb_defaults.is_empty());
    for key in ["condition", "favourited", "label", "swatch"] {
        assert!(
            metadata.item_defaults.contains_key(key),
            "missing saved-item default {key}"
        );
    }
    assert!(
        metadata.mood_history_length.is_some(),
        "missing mood array initializer"
    );
    assert!(
        metadata.character_identity.is_some(),
        "missing character identity generator"
    );
    // PlayerCamera.Start: Random.Range(130, 161) * Body.heightMultiplier.
    let heights: Vec<_> = (0..metadata.characters.len())
        .map(|character| schema::identity_range(0, character).unwrap())
        .collect();
    assert_eq!(heights, [130..=160, 104..=128, 143..=176]);
    for (character, height) in heights.iter().enumerate() {
        assert_eq!(schema::identity_range(1, character), Some(10..=25));
        let rolled = cu_save_editor::save::roll_identity(0, character).unwrap();
        assert!(height.contains(&rolled));
    }
    let experience = metadata.experience.as_ref().expect("missing XP curve");
    assert_eq!(experience.evaluate(0), Some(0));
    assert_eq!(experience.evaluate(13), Some(1062));
    assert_eq!(experience.evaluate(14), Some(1255));
    assert_eq!(experience.evaluate(15), Some(1484));

    let mut fields = std::collections::HashMap::from([
        ("weightOffset".into(), 0.0),
        ("sicknessAmount".into(), 0.0),
        ("hunger".into(), 100.0),
        ("thirst".into(), 100.0),
        ("skills.STR".into(), 10.0),
        ("skills.RES".into(), 10.0),
        ("encumbrancecap".into(), 1.0),
    ]);
    let capacity = metadata
        .carry_capacity
        .as_ref()
        .expect("missing carry expression");
    assert_eq!(capacity.evaluate_fields(None, &fields), Some(11.0));
    fields.insert("hunger".into(), 101.0);
    assert_eq!(capacity.evaluate_fields(None, &fields), Some(12.5));
    fields.insert("hunger".into(), 39.0);
    fields.insert("thirst".into(), 39.0);
    assert_eq!(capacity.evaluate_fields(None, &fields), Some(8.5));
    fields.remove("skills.RES");
    assert_eq!(capacity.evaluate_fields(None, &fields), None);

    for preset in schema::RUN_PRESETS {
        assert_eq!(
            schema::run_settings_for_preset(preset.id).unwrap().len(),
            metadata.run_settings.len(),
        );
    }
    let save = cu_save_editor::save::try_new_game_data().unwrap();
    cu_save_editor::save::validate(&save).unwrap();
    assert_eq!(
        save["limbs"].as_array().unwrap().len(),
        metadata.limbs.len()
    );
    assert_eq!(
        save["body"]["skills"][metadata.skills[0].key],
        metadata.characters[0].base_skills[0]
    );
    assert_eq!(
        save["character"], 0,
        "new save must have a selectable character"
    );
    // SaveSystem.TryLoadGame casts these unconditionally; a missing one aborts
    // applying the whole world state.
    for key in [
        "biome",
        "totalTraveled",
        "lootRarity",
        "trapRarity",
        "runSettings",
        "caloriesConsumed",
        "runTime",
        "cHeight",
        "cAge",
        "cId",
        "cVer",
        "lastHappiness",
        "savedRecipeData",
        "endlessLoops",
        "isMercRun",
        "cheatedSave",
    ] {
        assert!(!save[key].is_null(), "new save is missing {key}");
    }
    assert_eq!(save["isMercRun"], false);
    for (index, key) in schema::IDENTITY_KEYS.into_iter().enumerate() {
        let value = save[key].as_i64().unwrap();
        assert!(schema::identity_range(index, 0).unwrap().contains(&value));
    }
    assert_eq!(save["items"], serde_json::json!([]));
    assert_eq!(
        save["lastHappiness"].as_array().unwrap().len(),
        metadata.mood_history_length.unwrap()
    );
}

/// Detects game updates that rename or remove the progression keys the
/// editor relies on: every key must still be a string literal in the game.
#[test]
fn installed_game_info_and_progress_keys_match_the_game() {
    let Some(game_dir) = std::env::var_os("CU_GAME_DIR") else {
        eprintln!("skipped: CU_GAME_DIR is not set");
        return;
    };
    let info = GameInfo::read(Path::new(&game_dir));
    assert!(info.version.is_some(), "game version not found");
    assert!(
        info.unity_version
            .as_deref()
            .is_some_and(|v| v.starts_with("20")),
        "Unity version not found"
    );
    let assembly = std::fs::read(
        Path::new(&game_dir)
            .join("CasualtiesUnknown_Data")
            .join("Managed")
            .join("Assembly-CSharp.dll"),
    )
    .unwrap();
    let contains_utf16 = |text: &str| {
        let needle: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assembly
            .windows(needle.len())
            .any(|window| window == needle)
    };
    let mut keys = vec!["wonchar", "deepestlayer"];
    for ending in &schema::ENDINGS {
        keys.extend([ending.pref_key, ending.achievement, ending.status_key]);
    }
    for key in keys {
        assert!(contains_utf16(key), "the game no longer uses {key:?}");
    }
}

#[test]
fn item_and_container_metadata_is_loaded() {
    let Some(game_dir) = std::env::var_os("CU_GAME_DIR") else {
        eprintln!("skipped: CU_GAME_DIR is not set");
        return;
    };
    let stats = cu_save_editor::game_data::load_item_stats(Path::new(&game_dir)).unwrap();
    let path = Path::new(&game_dir)
        .join("CasualtiesUnknown_Data")
        .join("Managed")
        .join("Assembly-CSharp.dll");
    let assembly = dotscope::CilObject::from_path_with_validation(
        &path,
        dotscope::ValidationConfig::minimal(),
    )
    .unwrap();
    let mut lerp_found = false;
    for entry in assembly.methods().iter() {
        let method = entry.value();
        let owner = method
            .declaring_type
            .get()
            .and_then(|owner| owner.upgrade());
        if owner.is_some_and(|owner| owner.name == "Item") && method.name == "get_totalWeight" {
            for ins in method.instructions() {
                if let dotscope::assembly::Operand::Token(token) = &ins.operand
                    && let Some(member) = assembly.member_ref(token)
                {
                    lerp_found |= member.name == "Lerp";
                }
            }
        }
    }
    assert!(lerp_found, "game item weight no longer calls Lerp");
    let bag = stats.get("bigpack").expect("missing container bigpack");
    assert_eq!(bag.container_capacity, Some(7.5_f32 + 0.001));
    assert!(bag.container_capacity.is_some_and(|value| value > 0.0));
    assert!(
        bag.container_max_item_weight
            .is_some_and(|value| value > 0.0)
    );
    assert!(
        bag.container_encumbrance
            .is_some_and(|value| value.is_finite() && value >= 0.0)
    );
    assert!(
        stats.values().any(|item| item.trade_value > 0.0),
        "no item trade values were loaded"
    );
    let restricted_container = stats
        .values()
        .find(|item| {
            item.container_tag_restriction
                .as_ref()
                .is_some_and(|tags| !tags.is_empty())
        })
        .expect("no tag-restricted container was loaded");
    assert!(
        stats
            .values()
            .any(|item| { restricted_container.accepts_item(item) == Some(true) }),
        "no item matched the container's tag restrictions"
    );
    assert!(
        stats
            .values()
            .any(|item| { restricted_container.accepts_item(item) == Some(false) }),
        "container tag restrictions did not reject any item"
    );
    for id in ["waterbottle", "chocolatemilk", "combatpen", "liquidpouch"] {
        let item = stats
            .get(id)
            .unwrap_or_else(|| panic!("missing liquid item {id}"));
        assert!(item.liquid_capacity.is_some_and(|capacity| capacity > 0.0));
        assert!(item.weight > 0.0);
    }
    let toolbox = stats.get("toolbox").expect("missing toolbox");
    assert!(toolbox.only_hold_in_hands && toolbox.container_capacity.is_some());
    assert!(!bag.only_hold_in_hands);
    assert!(stats["autopump"].wearable && stats["autopump"].wearable_can_be_held);
    assert!(!stats["bigpack"].wearable_can_be_held);
    assert!(
        stats
            .values()
            .filter(|item| item.wearable)
            .all(|item| item.desired_wear_limb.is_some()),
        "every wearable should name the limb it is worn on"
    );
    assert!(stats.values().any(|item| item.destroy_at_zero_condition));
}

#[test]
fn item_and_liquid_effects_are_read_from_the_game_code() {
    use cu_save_editor::game_data::{Change, ContentsUse, Effect, Target};
    let Some(game_dir) = std::env::var_os("CU_GAME_DIR") else {
        eprintln!("skipped: CU_GAME_DIR is not set");
        return;
    };
    let data = cu_save_editor::game_data::load_game_data(Path::new(&game_dir)).unwrap();
    let effects = &data.effects;
    assert!(effects.items.values().any(|item| {
        item.qualities
            .iter()
            .any(|(id, amount)| id == "foliage" && *amount >= 1.0)
    }));
    assert!(effects.liquids.values().any(|liquid| {
        liquid
            .qualities
            .iter()
            .any(|(id, amount)| id == "disinfectant" && *amount >= 1.0)
    }));
    let add = |list: &[Effect], target: &Target, stat: &str| {
        list.iter()
            .filter(|e| &e.target == target && e.stat == stat && !e.conditional)
            .find_map(|e| match e.change {
                Change::Add(v) => Some(v),
                _ => None,
            })
    };
    let close =
        |value: Option<f32>, expected: f32| value.is_some_and(|v| (v - expected).abs() < 0.01);

    let bread = &effects.items["bread"].uses[0];
    assert!(close(add(&bread.effects, &Target::Body, "hunger"), 9.0));
    assert!(close(add(&bread.effects, &Target::Body, "thirst"), 2.0));
    assert!(close(bread.condition_per_use, 0.34));

    // A minigame bandage: amounts are for using up the whole bandage.
    let bandage = &effects.items["bandage"].uses[0];
    assert!(bandage.on_limb);
    let pain = bandage
        .effects
        .iter()
        .find(|e| e.stat == "pain")
        .expect("bandage pain");
    assert!(pain.whole_item && matches!(pain.change, Change::Add(v) if v < 0.0));

    let pills = &effects.items["painkillers"];
    assert_eq!(
        pills.uses[0].contents.map(|(kind, _)| kind),
        Some(ContentsUse::Drink)
    );
    assert!(
        pills
            .default_contents
            .iter()
            .any(|(id, _)| id == "painkillers")
    );
    let painkiller = Target::Component("Painkillers".into());
    assert!(close(
        add(
            &effects.liquids["painkillers"].drink,
            &painkiller,
            "opiateAmount"
        ),
        140.0
    ));
    assert_eq!(
        effects.liquids["water"].locale_name.as_deref(),
        Some("cleanwater")
    );
    // Random and overdose effects are kept apart from the steady ones.
    let stimulant = &effects.liquids["midgradestimulant"].inject;
    assert!(close(add(stimulant, &Target::Body, "stamina"), 720.0));
    assert!(stimulant.iter().any(|e| e.conditional));
}

#[test]
fn steam_achievements_of_this_computer_are_read() {
    use cu_save_editor::achievements::Achievements;
    use cu_save_editor::platform::STEAM_APP_ID;
    // Only meaningful where Steam has the game's statistics cached.
    let Ok(achievements) = Achievements::load(STEAM_APP_ID) else {
        return;
    };
    assert!(
        achievements.list.len() >= 40,
        "{} achievements",
        achievements.list.len()
    );
    let biopod = achievements
        .list
        .iter()
        .find(|a| a.api_name == "BIOPOD")
        .expect("BIOPOD");
    assert_eq!(biopod.name("english"), "Company's Best");
    assert!(biopod.hidden);
    if achievements.progress_found {
        assert!(
            achievements
                .list
                .iter()
                .all(|a| a.unlocked || a.unlocked_at.is_none())
        );
    }
}
