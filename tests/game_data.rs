//! Optional integration check against a locally installed game.

use std::path::Path;

#[test]
fn liquid_item_metadata_is_loaded() {
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
    for id in ["waterbottle", "chocolatemilk", "combatpen", "liquidpouch"] {
        let item = stats
            .get(id)
            .unwrap_or_else(|| panic!("missing liquid item {id}"));
        assert!(item.liquid_capacity.is_some_and(|capacity| capacity > 0.0));
        assert!(item.weight > 0.0);
    }
}
