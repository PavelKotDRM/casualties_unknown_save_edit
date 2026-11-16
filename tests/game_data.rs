//! Optional integration check against a locally installed game.

use std::path::Path;

use cu_save_editor::game_info::GameInfo;
use cu_save_editor::schema;

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
