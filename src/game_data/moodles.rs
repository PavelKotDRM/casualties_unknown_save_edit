//! Status names, native palette references and stat directions from MoodleManager.

use std::collections::BTreeMap;

use dotscope::CilObject;

use super::metadata::Reader;
use super::{StaticValue, instruction_string, instruction_value};
use crate::schema::Good;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MoodleData {
    pub neutral_colour: Option<[u8; 3]>,
    pub colours: BTreeMap<String, [u8; 3]>,
    pub directions: BTreeMap<String, Good>,
}

fn polarity([red, green, blue]: [u8; 3]) -> Option<bool> {
    if green > red && green > blue {
        Some(true)
    } else if red > green && green >= blue {
        Some(false)
    } else {
        None
    }
}

pub(super) fn parse(assembly: &CilObject, palette: &[[u8; 3]]) -> Result<MoodleData, String> {
    let reader = Reader::new(assembly);
    let mut result = MoodleData {
        neutral_colour: palette
            .iter()
            .find(|[red, green, blue]| red == green && green == blue)
            .copied(),
        ..Default::default()
    };
    let mut calls = 0;
    for entry in assembly.methods().iter() {
        let method = entry.value();
        if !method
            .declaring_type
            .get()
            .and_then(|owner| owner.upgrade())
            .is_some_and(|owner| owner.name == "MoodleManager")
        {
            continue;
        }
        let code: Vec<_> = method.instructions().collect();
        let mut start = 0;
        for (index, instruction) in code.iter().enumerate() {
            if !reader
                .callee(&instruction.operand)
                .is_some_and(|(owner, name)| owner == "MoodleManager" && name == "AddMoodle")
            {
                continue;
            }
            calls += 1;
            let name_call = (start..index).find(|&at| {
                reader
                    .callee(&code[at].operand)
                    .is_some_and(|(owner, name)| owner == "Locale" && name == "GetMoodle")
            });
            let definition = name_call.and_then(|at| {
                if at < 3 || code[at - 1].mnemonic != "ldstr" || code[at - 2].mnemonic != "ldstr" {
                    return None;
                }
                let key = at
                    .checked_sub(1)
                    .and_then(|at| instruction_string(code[at], assembly.userstrings()))?;
                let intensity = at
                    .checked_sub(3)
                    .and_then(|at| instruction_value(code[at], assembly.userstrings()));
                let Some(StaticValue::Number(intensity)) = intensity else {
                    return None;
                };
                if intensity < 0.0 || intensity.fract() != 0.0 {
                    return None;
                }
                let colour = *palette.get(intensity as usize)?;
                Some((key, colour, at.saturating_sub(2)))
            });
            if let Some((key, colour, arguments)) = definition {
                result.colours.insert(key, colour);
                let positive = polarity(colour);
                let [red, green, blue] = colour;
                if positive.is_some() || red != green || green != blue {
                    let mut guarded_field = None;
                    for at in (start + 2..arguments).rev() {
                        let guard = code[at];
                        if !guard
                            .branch_targets
                            .iter()
                            .any(|target| *target > instruction.rva)
                        {
                            continue;
                        }
                        let rising = if guard.mnemonic.starts_with("ble")
                            || guard.mnemonic.starts_with("blt")
                        {
                            true
                        } else if guard.mnemonic.starts_with("bge")
                            || guard.mnemonic.starts_with("bgt")
                        {
                            false
                        } else {
                            continue;
                        };
                        if !code[at - 1].mnemonic.starts_with("ldc.")
                            || code[at - 2].mnemonic != "ldfld"
                        {
                            continue;
                        }
                        if let Some((owner, field)) = reader.field(&code[at - 2].operand)
                            && matches!(owner.as_str(), "Body" | "Limb")
                        {
                            let key = format!("{owner}.{field}");
                            if guarded_field
                                .as_ref()
                                .is_some_and(|previous| previous != &key)
                            {
                                continue;
                            }
                            guarded_field = Some(key.clone());
                            let direction = match positive {
                                Some(positive) if positive == rising => Good::High,
                                Some(_) => Good::Low,
                                None => Good::Neutral,
                            };
                            result
                                .directions
                                .entry(key)
                                .and_modify(|previous| {
                                    if *previous != direction {
                                        *previous = Good::Neutral;
                                    }
                                })
                                .or_insert(direction);
                        }
                    }
                }
            }
            start = index + 1;
        }
    }
    if calls == 0 || result.colours.is_empty() {
        return Err("Cannot read MoodleManager.AddMoodle status palette references".into());
    }
    Ok(result)
}
