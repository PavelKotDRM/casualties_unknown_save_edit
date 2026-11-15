//! Optional integration check against a locally installed game.

use std::path::Path;

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
