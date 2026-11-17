//! Game metadata read from an installed assembly and Unity assets without
//! loading or executing game code. No installation means no metadata.

use dotscope::{CilObject, ValidationConfig};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub mod ammunition;
mod assets;
pub mod effects;
mod il;
pub mod metadata;
pub mod moodles;
pub mod program;
pub mod runtime;

pub use effects::{
    Change, ContentsUse, Effect, GameEffects, ItemProfile, ItemUse, LiquidProfile, Target,
};
use il::{
    StaticValue, bool_field, instruction_string, instruction_value, number as number_field,
    text as string_field,
};

/// Verified item fields read from the installed game.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemStats {
    pub category: String,
    pub weight: f32,
    pub scale_weight_with_condition: bool,
    pub wearable: bool,
    pub wear_slot: Option<String>,
    pub armor: Option<f32>,
    pub insulation: Option<f32>,
    pub item_tags: HashSet<String>,
    pub container_capacity: Option<f32>,
    pub container_max_item_weight: Option<f32>,
    pub container_encumbrance: Option<f32>,
    pub container_tag_restriction: Option<HashSet<String>>,
    pub liquid_capacity: Option<f32>,
    pub only_hold_in_hands: bool,
    pub wearable_can_be_held: bool,
    pub desired_wear_limb: Option<String>,
    pub destroy_at_zero_condition: bool,
}

impl ItemStats {
    /// Returns NaN if condition scaling requires an unavailable game formula.
    pub fn total_weight(&self, condition: f32) -> f32 {
        if self.scale_weight_with_condition {
            let Some(minimum) = runtime::metadata().minimum_item_weight else {
                return f32::NAN;
            };
            self.weight_with_minimum(condition, minimum)
        } else {
            self.weight
        }
    }

    fn weight_with_minimum(&self, condition: f32, minimum: f32) -> f32 {
        minimum + (self.weight - minimum) * condition.clamp(0.0, 1.0)
    }

    pub fn accepts_item(&self, item: &ItemStats) -> Option<bool> {
        let restrictions = self.container_tag_restriction.as_ref()?;
        Some(restrictions.is_empty() || restrictions.iter().any(|tag| item.item_tags.contains(tag)))
    }
}

#[derive(Debug, Clone, Default)]
pub struct GameData {
    pub items: HashMap<String, ItemStats>,
    pub effects: GameEffects,
    pub metadata: metadata::Metadata,
}

pub fn load_item_stats(game_dir: &Path) -> Result<HashMap<String, ItemStats>, String> {
    load_game_data(game_dir).map(|data| data.items)
}

/// Parses the installation and publishes its immutable compatibility snapshot.
/// On error the caller should clear the old selection with [`runtime::clear`].
pub fn load_game_data(game_dir: &Path) -> Result<GameData, String> {
    let data_dir = game_dir.join("CasualtiesUnknown_Data");
    let assembly_path = data_dir.join("Managed").join("Assembly-CSharp.dll");
    let assembly =
        CilObject::from_path_with_validation(&assembly_path, ValidationConfig::minimal())
            .map_err(|error| format!("Cannot parse {}: {error}", assembly_path.display()))?;
    let effects = effects::parse(&assembly);
    let mut items = il::items(&assembly)?;
    assets::containers(&data_dir, &assembly, &mut items)?;
    let locale = crate::locale::Locale::load(&data_dir.join("Lang").join("EN.json"));
    let mut metadata = metadata::parse(
        &assembly,
        &items,
        effects.liquids.keys().cloned(),
        locale.as_ref(),
    );
    assets::player(&data_dir, &assembly, &mut metadata, locale.as_ref())?;
    metadata::apply_awake_defaults(&assembly, &mut metadata);
    metadata::refresh_targets(&mut metadata);
    match assets::ammunition(&data_dir, &assembly) {
        Ok(ammunition) => metadata.ammunition = Some(ammunition),
        Err(error) => metadata.ammunition_error = Some(error),
    }
    match assets::moodle_palette(&data_dir).and_then(|palette| moodles::parse(&assembly, &palette))
    {
        Ok(moodles) => metadata.moodles = moodles,
        Err(error) => metadata.moodles_error = Some(error),
    }
    runtime::install(metadata.clone());
    Ok(GameData {
        items,
        effects,
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(scaled: bool) -> ItemStats {
        ItemStats {
            category: "tool".into(),
            weight: 2.0,
            scale_weight_with_condition: scaled,
            wearable: false,
            wear_slot: None,
            armor: None,
            insulation: None,
            item_tags: HashSet::new(),
            container_capacity: None,
            container_max_item_weight: None,
            container_encumbrance: None,
            container_tag_restriction: None,
            liquid_capacity: None,
            only_hold_in_hands: false,
            wearable_can_be_held: false,
            desired_wear_limb: None,
            destroy_at_zero_condition: false,
        }
    }

    #[test]
    fn item_weight_applies_condition_scaling_only_when_defined() {
        let scaled = item(true);
        assert!((scaled.weight_with_minimum(0.0, 0.2) - 0.2).abs() < f32::EPSILON);
        assert!((scaled.weight_with_minimum(0.5, 0.2) - 1.1).abs() < f32::EPSILON);
        assert_eq!(scaled.weight_with_minimum(2.0, 0.2), 2.0);
        assert_eq!(item(false).total_weight(0.0), 2.0);
    }

    #[test]
    fn container_tag_restrictions_match_any_shared_item_tag() {
        let mut container = item(false);
        container.container_tag_restriction =
            Some(HashSet::from(["bullet".into(), "battery".into()]));
        let mut allowed = item(false);
        allowed.item_tags = HashSet::from(["ammo".into(), "bullet".into()]);
        assert_eq!(container.accepts_item(&allowed), Some(true));
        assert_eq!(container.accepts_item(&item(false)), Some(false));
        container.container_tag_restriction = Some(HashSet::new());
        assert_eq!(container.accepts_item(&item(false)), Some(true));
        assert_eq!(item(false).accepts_item(&allowed), None);
    }
}
