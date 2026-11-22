//! Game metadata read from an installed assembly and Unity assets without
//! loading or executing game code. No installation means no metadata.

use dotscope::{CilObject, ValidationConfig};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub mod ammunition;
mod assets;
pub mod crafting;
pub mod drops;
pub mod effects;
mod il;
pub mod metadata;
pub mod moodles;
pub mod program;
pub mod protection;
pub mod runtime;
pub mod traders;

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
    pub trade_value: f32,
    pub untradeable: bool,
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
    pub crafting: Vec<crafting::CraftingRecipe>,
    pub crafting_icons: HashMap<String, crafting::CraftingIcon>,
    pub crafting_icons_error: Option<String>,
    pub drop_sources: Vec<drops::DropSource>,
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
    let crafting = crafting::parse(&assembly)?;
    let mut items = il::items(&assembly)?;
    let item_ids = items
        .keys()
        .cloned()
        .chain(crate::catalog::ITEMS.iter().map(|item| item.id.to_owned()));
    let (crafting_icons, crafting_icons_error) =
        assets::crafting_icons(&data_dir, &crafting, item_ids);
    let drop_sources = assets::drop_sources(&data_dir, &assembly)?;
    assets::containers(&data_dir, &assembly, &mut items)?;
    let locale = crate::locale::Locale::load(&data_dir.join("Lang").join("EN.json"));
    let mut metadata = metadata::parse(
        &assembly,
        &items,
        effects.liquids.keys().cloned(),
        locale.as_ref(),
    );
    assets::player(&data_dir, &assembly, &mut metadata, locale.as_ref())?;
    match protection::load(&data_dir, &assembly, &metadata, &items) {
        Ok(protection) => metadata.protection = Some(protection),
        Err(error) => metadata.protection_error = Some(error),
    }
    metadata::apply_awake_defaults(&assembly, &mut metadata);
    metadata::refresh_targets(&mut metadata);
    match assets::ammunition(&data_dir, &assembly) {
        Ok(ammunition) => metadata.ammunition = Some(ammunition),
        Err(error) => metadata.ammunition_error = Some(error),
    }
    match assets::batteries(&data_dir, &assembly) {
        Ok(batteries) => {
            for (id, defaults) in batteries {
                metadata
                    .battery_limits
                    .insert(id.clone(), defaults.max_allowed_charge);
                metadata.item_components.insert(
                    id,
                    serde_json::json!({component_key("BatteryItem"): defaults.state}),
                );
            }
        }
        Err(error) => metadata.item_components_error = Some(error),
    }
    for (id, profile) in &effects.items {
        if let Some(charge) = profile.max_charge {
            metadata.battery_charges.insert(id.clone(), charge);
        }
        let uses: Vec<_> = profile
            .uses
            .iter()
            .filter_map(|action| action.condition_use)
            .collect();
        if !uses.is_empty() {
            metadata.condition_uses.insert(id.clone(), uses);
        }
    }
    for (id, item) in &items {
        if item.liquid_capacity.is_some() {
            let Some(profile) = effects.items.get(id) else {
                metadata.item_components_error =
                    Some(format!("Liquid defaults for {id} are unavailable"));
                continue;
            };
            let stack: Vec<_> = profile
                .default_contents
                .iter()
                .map(|(liquid, amount)| serde_json::json!({"liquidId": liquid, "amount": amount}))
                .collect();
            metadata
                .item_components
                .entry(id.clone())
                .or_insert_with(|| serde_json::json!({}))[component_key("WaterContainerItem")] =
                serde_json::json!({"stack": stack});
        }
    }
    if let Some(ammunition) = &metadata.ammunition {
        for (id, owner, defaults) in ammunition
            .items
            .iter()
            .map(|(id, item)| (id, "AmmoScript", &item.defaults))
            .chain(
                ammunition
                    .guns
                    .iter()
                    .map(|(id, gun)| (id, "GunScript", &gun.defaults)),
            )
        {
            metadata
                .item_components
                .entry(id.clone())
                .or_insert_with(|| serde_json::json!({}))[component_key(owner)] =
                serde_json::json!(defaults);
        }
    }
    assign_item_groups(&mut metadata, &items, &effects);
    match assets::moodle_palette(&data_dir).and_then(|palette| moodles::parse(&assembly, &palette))
    {
        Ok(moodles) => metadata.moodles = moodles,
        Err(error) => metadata.moodles_error = Some(error),
    }

    runtime::install(metadata.clone());
    Ok(GameData {
        items,
        effects,
        crafting,
        crafting_icons,
        crafting_icons_error,
        drop_sources,
        metadata,
    })
}

pub fn component_key(owner: &str) -> String {
    format!("{owner}, Assembly-CSharp, Version=0.0.0.0, Culture=neutral, PublicKeyToken=null")
}

fn assign_item_groups(
    metadata: &mut metadata::Metadata,
    items: &HashMap<String, ItemStats>,
    effects: &GameEffects,
) {
    for def in &mut metadata.items {
        let Some(stats) = items.get(def.id) else {
            continue;
        };
        let ammunition = metadata.ammunition.as_ref().is_some_and(|ammunition| {
            ammunition.items.contains_key(def.id) || ammunition.guns.contains_key(def.id)
        });
        let profile = effects.items.get(def.id);
        let edible = profile.is_some_and(|profile| {
            profile
                .uses
                .iter()
                .flat_map(|item_use| &item_use.effects)
                .any(|effect| {
                    effect.target == Target::Body
                        && match effect.stat.as_str() {
                            "hunger" | "thirst" => {
                                matches!(effect.change, Change::Add(amount) if amount > 0.0)
                            }
                            // Set while chewing; only food has it.
                            "eatTime" => true,
                            _ => false,
                        }
                })
        });
        let drinkable = profile.is_some_and(|profile| {
            profile
                .uses
                .iter()
                .any(|item_use| matches!(item_use.contents, Some((ContentsUse::Drink, _))))
        });
        def.group = crate::catalog::classify(&crate::catalog::GroupFacts {
            category: &stats.category,
            tags: &stats.item_tags,
            worn: stats.wear_slot.is_some(),
            bag: stats.wear_slot.is_some()
                && stats.container_capacity.is_some()
                && stats.armor.is_none_or(|armor| armor <= 0.0)
                && stats.insulation.is_none_or(|insulation| insulation <= 0.0),
            ammunition,
            edible,
            drinkable,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(scaled: bool) -> ItemStats {
        ItemStats {
            category: "tool".into(),
            weight: 2.0,
            trade_value: 0.0,
            untradeable: false,
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
