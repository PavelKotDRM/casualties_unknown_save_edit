//! A bounded interpreter for scalar IL expressions. It accepts no arbitrary
//! game calls and performs no IO; unsupported instructions fail closed.

use std::collections::HashMap;

use dotscope::CilObject;
use dotscope::assembly::{Instruction, Operand};

use super::metadata::Reader;
use super::{StaticValue, instruction_value};

#[derive(Debug, Clone, PartialEq)]
struct Step {
    code: &'static str,
    number: Option<f32>,
    text: String,
    target: Option<usize>,
    local: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScalarProgram {
    steps: Vec<Step>,
}

#[derive(Clone)]
enum Value {
    Number(f32),
    Path(String),
}

impl Value {
    fn number(self) -> Option<f32> {
        match self {
            Self::Number(value) => Some(value),
            _ => None,
        }
    }
}

impl ScalarProgram {
    #[cfg(test)]
    pub(crate) fn test_linear(field: Option<&str>, scale: f32, offset: f32) -> Self {
        let step = |code, number, text: &str| Step {
            code,
            number,
            text: text.into(),
            target: None,
            local: 0,
        };
        let mut steps = vec![step("ldarg.0", None, "")];
        if let Some(field) = field {
            steps.push(step("ldfld", None, field));
        }
        steps.extend([
            step("ldc.r4", Some(scale), ""),
            step("mul", None, ""),
            step("ldc.r4", Some(offset), ""),
            step("add", None, ""),
            step("ret", None, ""),
        ]);
        Self { steps }
    }

    pub(super) fn read(assembly: &CilObject, instructions: &[&Instruction]) -> Option<Self> {
        let reader = Reader::new(assembly);
        let mut steps = Vec::new();
        for instruction in instructions {
            let value = instruction_value(instruction, assembly.userstrings());
            let mut step = Step {
                code: instruction.mnemonic,
                number: match &value {
                    Some(StaticValue::Number(number)) => Some(*number),
                    _ => None,
                },
                text: match value {
                    Some(StaticValue::String(text)) => text,
                    _ => String::new(),
                },
                target: instruction.branch_targets.first().and_then(|target| {
                    instructions
                        .iter()
                        .position(|instruction| instruction.rva == *target)
                }),
                local: match instruction.operand {
                    Operand::Local(index) | Operand::Argument(index) => index as usize,
                    Operand::Immediate(dotscope::assembly::Immediate::UInt8(index)) => {
                        index as usize
                    }
                    Operand::Immediate(dotscope::assembly::Immediate::UInt16(index)) => {
                        index as usize
                    }
                    _ => instruction
                        .mnemonic
                        .rsplit('.')
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0),
                },
            };
            if matches!(step.code, "call" | "callvirt") {
                let (owner, name) = reader.callee(&instruction.operand)?;
                step.text = format!("{owner}.{name}");
                if name.ends_with("From10")
                    && let Some(method) = reader.method(&owner, &name)
                {
                    let body: Vec<_> = method.instructions().collect();
                    // Property getter: this.<skill> converted to float minus
                    // a literal baseline. Do not assume the baseline is 10.
                    if body.len() == 6
                        && body[1].mnemonic == "ldfld"
                        && body[3].mnemonic.starts_with("ldc.")
                        && body[4].mnemonic == "sub"
                    {
                        step.number =
                            instruction_value(body[3], assembly.userstrings()).and_then(|value| {
                                match value {
                                    StaticValue::Number(n) => Some(n),
                                    _ => None,
                                }
                            });
                        step.text = reader.field(&body[1].operand)?.1;
                        step.code = "getter.offset";
                    }
                }
            } else if matches!(step.code, "ldfld" | "stfld") {
                step.text = reader.field(&instruction.operand)?.1;
            }
            steps.push(step);
        }
        Some(Self { steps })
    }

    pub fn evaluate(&self, argument: i32) -> Option<i64> {
        self.evaluate_fields(Some(argument as f32), &HashMap::new())
            .filter(|value| {
                value.is_finite() && *value >= i64::MIN as f32 && *value < i64::MAX as f32
            })
            .map(|value| value as i64)
    }

    /// `fields` uses dotted paths such as `skills.STR`, plus run-setting IDs.
    pub fn evaluate_fields(
        &self,
        argument: Option<f32>,
        fields: &HashMap<String, f32>,
    ) -> Option<f32> {
        let mut stack: Vec<Value> = Vec::new();
        let mut locals: HashMap<usize, Value> = HashMap::new();
        let mut pc = 0usize;
        // A malformed/new game assembly cannot hang the editor.
        for _ in 0..100_000 {
            let step = self.steps.get(pc)?;
            pc += 1;
            let code = step.code;
            if code.starts_with("ldc.") {
                stack.push(Value::Number(step.number?));
            } else if code.starts_with("ldloc") {
                stack.push(locals.get(&step.local)?.clone());
            } else if code.starts_with("stloc") {
                locals.insert(step.local, stack.pop()?);
            } else if code.starts_with("ldarg") {
                stack.push(
                    argument
                        .map(Value::Number)
                        .unwrap_or_else(|| Value::Path(String::new())),
                );
            } else if code.starts_with("conv.r") || code == "nop" {
            } else if code.starts_with("conv.i") {
                let value = stack.pop()?.number()?;
                stack.push(Value::Number(value.trunc()));
            } else {
                match code {
                    "ldstr" => stack.push(Value::Path(step.text.clone())),
                    "dup" => stack.push(stack.last()?.clone()),
                    "pop" => {
                        stack.pop()?;
                    }
                    "ldfld" | "getter.offset" => {
                        let Value::Path(path) = stack.pop()? else {
                            return None;
                        };
                        let path = if path.is_empty() {
                            step.text.clone()
                        } else {
                            format!("{path}.{}", step.text)
                        };
                        if let Some(value) = fields.get(&path) {
                            stack.push(Value::Number(*value - step.number.unwrap_or(0.0)));
                        } else if code == "ldfld" {
                            stack.push(Value::Path(path));
                        } else {
                            return None;
                        }
                    }
                    "add" | "sub" | "mul" | "div" | "ceq" | "cgt" | "clt" => {
                        let right = stack.pop()?.number()?;
                        let left = stack.pop()?.number()?;
                        let value = match code {
                            "add" => left + right,
                            "sub" => left - right,
                            "mul" => left * right,
                            "div" => left / right,
                            "ceq" => (left == right) as u8 as f32,
                            "cgt" => (left > right) as u8 as f32,
                            _ => (left < right) as u8 as f32,
                        };
                        stack.push(Value::Number(value));
                    }
                    "neg" => {
                        let value = -stack.pop()?.number()?;
                        stack.push(Value::Number(value));
                    }
                    "br" | "br.s" => pc = step.target?,
                    "brtrue" | "brtrue.s" | "brfalse" | "brfalse.s" => {
                        let yes = stack.pop()?.number()? != 0.0;
                        if yes == code.starts_with("brtrue") {
                            pc = step.target?;
                        }
                    }
                    "ble" | "ble.s" | "blt" | "blt.s" | "bge" | "bge.s" | "bgt" | "bgt.s"
                    | "beq" | "beq.s" | "bne.un" | "bne.un.s" => {
                        let right = stack.pop()?.number()?;
                        let left = stack.pop()?.number()?;
                        let branch = match code.split('.').next()? {
                            "ble" => left <= right,
                            "blt" => left < right,
                            "bge" => left >= right,
                            "bgt" => left > right,
                            "beq" => left == right,
                            _ => left != right,
                        };
                        if branch {
                            pc = step.target?;
                        }
                    }
                    "call" | "callvirt" => {
                        let value = match step.text.as_str() {
                            "Mathf.RoundToInt" => {
                                let value = stack.pop()?.number()?.round_ties_even();
                                if !value.is_finite()
                                    || value < i32::MIN as f32
                                    || value >= i32::MAX as f32
                                {
                                    return None;
                                }
                                value
                            }
                            "Mathf.Min" => {
                                let right = stack.pop()?.number()?;
                                stack.pop()?.number()?.min(right)
                            }
                            "Mathf.Clamp" => {
                                let max = stack.pop()?.number()?;
                                let min = stack.pop()?.number()?;
                                if min > max {
                                    return None;
                                }
                                stack.pop()?.number()?.clamp(min, max)
                            }
                            "WorldGeneration.GetRunSettingFloat" => {
                                let Value::Path(key) = stack.pop()? else {
                                    return None;
                                };
                                *fields.get(&key)?
                            }
                            _ => return None,
                        };
                        stack.push(Value::Number(value));
                    }
                    "ret" | "stfld" => {
                        return stack.pop()?.number().filter(|value| value.is_finite());
                    }
                    _ => return None,
                }
            }
            if stack.len() > 256 || locals.len() > 256 {
                return None;
            }
        }
        None
    }
}
