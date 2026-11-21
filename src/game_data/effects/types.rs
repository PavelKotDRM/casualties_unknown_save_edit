//! Public item and liquid effect data.

use super::*;

/// Which object a change applies to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Target {
    /// The whole body.
    Body,
    /// The limb the item is used on.
    TreatedLimb,
    /// A specific limb by index into the body's limb array.
    Limb(usize),
    /// Every limb.
    EachLimb,
    /// A body component, such as `Painkillers`.
    Component(String),
}

/// How a stat changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Change {
    /// Adds the amount (negative values subtract).
    Add(f32),
    /// Multiplies the stat.
    Multiply(f32),
    /// Sets the stat to the value.
    Set(f32),
    /// An event without an amount, such as vomiting.
    Trigger,
}

/// One stat change.
#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    pub target: Target,
    /// Game field name (`pain`, `thirst`…) or a pseudo stat (`disinfect`, `vomit`).
    pub stat: String,
    pub change: Change,
    /// Applied gradually, once per second, over this many seconds.
    pub seconds: Option<f32>,
    /// The amount is for using up a whole item (in a minigame), rather than
    /// for a single use.
    pub whole_item: bool,
    /// Happens only under some condition (randomly, on overdose…); the
    /// amount is for one occurrence.
    pub conditional: bool,
}

/// How an item uses the liquid inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentsUse {
    Drink,
    Inject,
    ApplyToLimb,
}

/// A fixed condition subtraction and its native condition guard.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConditionUse {
    pub on_limb: bool,
    pub cost: f32,
    pub minimum: f32,
    pub inclusive: bool,
}

/// What using an item does.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemUse {
    /// Used on a limb (`useLimbAction`) rather than on the body.
    pub on_limb: bool,
    pub effects: Vec<Effect>,
    /// Item condition used up by one use (1 = the whole item).
    pub condition_per_use: Option<f32>,
    /// Present only for directly verified fixed-cost actions, not minigames.
    pub condition_use: Option<ConditionUse>,
    /// Liquid contents used per use, in millilitres.
    pub contents: Option<(ContentsUse, f32)>,
}

/// Extra item data from `Item.SetupItems`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemProfile {
    /// Trade value.
    pub value: Option<f32>,
    pub max_charge: Option<f32>,
    /// Minutes until the item spoils.
    pub decay_minutes: Option<f32>,
    /// INT skill needed to recognise the item.
    pub recognition: Option<i32>,
    /// Crafting qualities with their amounts.
    pub qualities: Vec<(String, f32)>,
    /// Liquids the item starts with, in millilitres.
    pub default_contents: Vec<(String, f32)>,
    pub combineable: bool,
    /// Usable while depressed.
    pub ignore_depression: bool,
    pub untradeable: bool,
    /// Change of jump height while worn.
    pub jump_height_change: Option<f32>,
    pub uses: Vec<ItemUse>,
}

/// A liquid from the game's `Liquids.Registry`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiquidProfile {
    /// Key of the display name in the game's language files.
    pub locale_name: Option<String>,
    pub value_per_liter: Option<f32>,
    pub injectable: bool,
    /// Sickness caused per millilitre injected is `0.35 ×` this.
    pub injection_sickness: Option<f32>,
    pub qualities: Vec<(String, f32)>,
    /// Effects of drinking 100 ml.
    pub drink: Vec<Effect>,
    /// Effects of injecting 100 ml.
    pub inject: Vec<Effect>,
}

/// Effects of everything the player can use.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GameEffects {
    pub items: HashMap<String, ItemProfile>,
    pub liquids: BTreeMap<String, LiquidProfile>,
}

impl GameEffects {
    /// Effects of `ml` millilitres of an item's contents, mixed in the
    /// proportions the item starts with.
    pub fn contents_effects(&self, item: &str, ml: f32, inject: bool) -> Vec<Effect> {
        let Some(profile) = self.items.get(item) else {
            return Vec::new();
        };
        let total: f32 = profile.default_contents.iter().map(|(_, v)| v).sum();
        if total <= 0.0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (liquid, amount) in &profile.default_contents {
            let Some(liquid) = self.liquids.get(liquid) else {
                continue;
            };
            let scale = ml * amount / total / LIQUID_EFFECT_VOLUME_ML;
            let effects = if inject {
                &liquid.inject
            } else {
                &liquid.drink
            };
            out.extend(effects.iter().map(|e| e.scaled(scale)));
        }
        merge(out)
    }
}

impl Effect {
    pub(super) fn scaled(&self, factor: f32) -> Self {
        let change = match self.change {
            Change::Add(v) => Change::Add(v * factor),
            other => other,
        };
        Self {
            change,
            seconds: self.seconds.map(|s| s * factor),
            ..self.clone()
        }
    }
}
