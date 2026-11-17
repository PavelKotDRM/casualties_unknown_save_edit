//! A separate test process verifies the initial state has no installed-game
//! metadata, regardless of optional installed-game tests in other binaries.

use cu_save_editor::{catalog, game_data, save, schema};
use serde_json::json;

#[test]
fn unavailable_game_leaves_runtime_features_empty_but_preserves_raw_saves() {
    game_data::runtime::clear();
    assert!(catalog::ITEMS.is_empty());
    assert!(catalog::LIQUIDS.is_empty());
    assert!(catalog::item("unknown-item").is_none());
    assert!(schema::CHARACTERS.is_empty());
    assert!(schema::ENDINGS.is_empty());
    assert!(schema::HOLD_SLOTS.is_empty());
    assert!(schema::SKILLS.is_empty());
    assert!(schema::LIMBS.is_empty());
    assert!(schema::LIMB_FIELDS.is_empty());
    assert!(schema::BODY_GROUPS.is_empty());
    assert!(schema::RUN_SETTINGS.is_empty());
    assert!(schema::RUN_PRESETS.is_empty());
    assert!(schema::LAYERS.is_empty());
    assert!(game_data::runtime::metadata().ammunition.is_none());
    assert_eq!(schema::experience_for_level(13), None);
    assert!(schema::run_settings_for_preset("normal").is_none());
    assert!(game_data::load_game_data(Path::new("nonexistent-game-directory")).is_err());
    assert!(catalog::ITEMS.is_empty());
    assert!(!save::can_create_new_game());
    assert!(save::new_game_data().is_err());

    let document = json!({
        "body": {"unknown-stat": 123.25},
        "limbs": [{"unknown-limb-stat": true}],
        "items": [{"id": "unknown-item", "other": ["preserved"]}],
        "unknown-root": "preserved"
    });
    let mut unchanged = document.clone();
    unchanged["itemComponents"] = json!([{}]);
    let before = unchanged.clone();
    assert!(cu_save_editor::model::reload_ammunition(&mut unchanged, 0).is_err());
    assert_eq!(unchanged, before);
    for format in [save::SaveFormat::PlainJson, save::SaveFormat::Gzip] {
        assert_eq!(
            save::decode(&save::encode(&document, format)).unwrap().0,
            document
        );
    }
}

use std::path::Path;
