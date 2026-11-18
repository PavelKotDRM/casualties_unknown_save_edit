//! Optional integration check against a locally installed game.

use std::path::Path;

use cu_save_editor::game_info::GameInfo;
use cu_save_editor::schema;

#[test]
fn runtime_catalogues_and_rules_come_from_the_installed_game() {
    let Some(game_dir) = std::env::var_os("CU_GAME_DIR") else {
        eprintln!("skipped: CU_GAME_DIR is not set");
        return;
    };
    let data = cu_save_editor::game_data::load_game_data(Path::new(&game_dir)).unwrap();
    let metadata = &data.metadata;
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
    assert_eq!(save["character"], 0, "new save must have a selectable character");
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
