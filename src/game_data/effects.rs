//! What using an item or a liquid does, read from the game's IL without
//! executing it.
//!
//! Item and liquid definitions (`Item.SetupItems`, the `Liquids` registry)
//! attach C# lambdas such as `useAction` and `onDrink`. Their bodies are
//! simple arithmetic on body and limb fields (`body.thirst += ml * 0.09f`),
//! calls such as `Body.Eat`, timed operations and minigame callbacks. This
//! module evaluates those bodies symbolically and reports the stat changes.
//! It is a best-effort reading: branches are not followed, so conditional
//! effects are listed together, and anything it cannot follow is skipped.

use std::collections::{BTreeMap, HashMap, HashSet};

use dotscope::CilObject;
use dotscope::assembly::{Immediate, Instruction, Operand};
use dotscope::metadata::method::MethodRc;
use dotscope::metadata::signatures::{SignatureMethod, TypeSignature};
use dotscope::metadata::tables::MemberRefSignature;
use dotscope::metadata::token::Token;
use dotscope::metadata::typesystem::CilTypeReference;

use super::{
    StaticValue, bool_field, instruction_string, instruction_value, number_field, string_field,
};

/// Editor reporting unit, not a game-defined dose or a default argument.
/// All liquid profiles use this volume so different liquids can be compared.
pub const LIQUID_EFFECT_VOLUME_ML: f32 = 100.0;

mod eval;
mod output;
mod reader;
mod types;

use eval::*;
use output::*;
use reader::*;
pub use types::*;

/// Reads item and liquid effects. Never fails: whatever cannot be read is left out.
pub(super) fn parse(assembly: &CilObject) -> GameEffects {
    let reader = Reader::new(assembly);
    let mut effects = GameEffects::default();
    if let Some(method) = reader.find_method("Item", "SetupItems") {
        for def in reader.definitions(&method, &["ItemInfo", "LiquidItemInfo"]) {
            effects
                .items
                .insert(def.id.clone(), reader.item_profile(&def));
        }
    }
    if let Some(method) = reader.find_method("Liquids", ".cctor") {
        for def in reader.definitions(&method, &["LiquidType"]) {
            effects
                .liquids
                .insert(def.id.clone(), reader.liquid_profile(&def));
        }
    }
    effects
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_of(effect: &Effect) -> f32 {
        match effect.change {
            Change::Add(v) => v,
            other => panic!("expected an addition, got {other:?}"),
        }
    }

    #[test]
    fn linear_arithmetic_tracks_field_changes() {
        // painkillers: opiateAmount += ml * 0.01 * 140
        let per_100 = arithmetic("mul", Val::Num(Lin::X), Val::Num(Lin::constant(0.01)));
        let amount = arithmetic("mul", per_100, Val::Num(Lin::constant(140.0)));
        let changed = arithmetic("add", Val::Read(7), amount);
        let mut out = Out::default();
        out.store(
            "Painkillers",
            "opiateAmount",
            7,
            &Val::Obj(Obj::Any),
            changed,
        );
        let effects = out.at(100.0).effects();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].target, Target::Component("Painkillers".into()));
        assert!((add_of(&effects[0]) - 140.0).abs() < 1e-3);
    }

    #[test]
    fn minigame_effects_are_scaled_to_a_whole_item() {
        // bandage: num2 = x / 12; item.condition -= num2; limb.pain -= num2 * 60
        let num2 = Lin::X.scale(1.0 / 12.0);
        let mut out = Out::default();
        let any = Val::Obj(Obj::Any);
        out.store(
            "Item",
            "condition",
            1,
            &any,
            Val::Rel(1, RawChange::Add(num2.scale(-1.0))),
        );
        out.store(
            "Limb",
            "pain",
            2,
            &any,
            Val::Rel(2, RawChange::Add(num2.scale(-60.0))),
        );
        let effects = out.normalize(true).effects();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].target, Target::TreatedLimb);
        assert!(effects[0].whole_item);
        assert!((add_of(&effects[0]) + 60.0).abs() < 1e-3);
    }

    #[test]
    fn timed_operations_total_their_ticks() {
        let mut tick = Out::default();
        let step = RawChange::Add(Lin::constant(-4.5));
        tick.add(Target::TreatedLimb, "bleedAmount", step);
        let mut out = Out::default();
        out.merge_timed(tick, Lin::constant(10.0));
        let effects = out.effects();
        assert_eq!(effects[0].seconds, Some(10.0));
        assert!((add_of(&effects[0]) + 45.0).abs() < 1e-3);
    }
}
