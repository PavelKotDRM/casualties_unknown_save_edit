//! Crafting recipes read from the game's `Recipes.SetUpRecipes` IL.

use std::collections::HashMap;

use dotscope::CilObject;
use dotscope::assembly::Instruction;

use super::metadata::Reader;
use super::{StaticValue, instruction_string, instruction_value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipeCategory {
    Materials,
    Tools,
    Medicine,
    Utilities,
    Food,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CraftingIngredient {
    pub is_liquid: bool,
    pub specific_id: Option<String>,
    pub quality_id: Option<String>,
    pub quality_amount: f32,
    pub minimum_condition: f32,
    pub destroy_item: bool,
    pub ignored_id: Option<String>,
    pub no_max_condition: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CraftingRecipe {
    pub index: usize,
    pub intelligence: i32,
    pub category: RecipeCategory,
    pub special_known: bool,
    pub is_repair: bool,
    pub result_id: String,
    pub result_is_liquid: bool,
    pub result_amount: i32,
    pub result_condition: f32,
    pub dont_drain_result_liquid: bool,
    pub ingredients: Vec<CraftingIngredient>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftingIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Number(f32),
    String(String),
    Object(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectKind {
    List,
    Recipe,
    Result,
    Ingredient,
    Quality,
}

#[derive(Debug, Default)]
struct Object {
    kind: Option<ObjectKind>,
    fields: HashMap<String, Value>,
    values: Vec<Value>,
}

pub fn parse(assembly: &CilObject) -> Result<Vec<CraftingRecipe>, String> {
    let reader = Reader::new(assembly);
    let method = reader
        .method("Recipes", "SetUpRecipes")
        .ok_or("Assembly-CSharp.dll has no Recipes.SetUpRecipes method")?;
    let mut objects = Vec::<Object>::new();
    let mut stack = Vec::<Value>::new();
    let mut static_fields = HashMap::<(String, String), Value>::new();
    let code: Vec<_> = method.instructions().collect();
    let mut registry_end = None;

    for (index, instruction) in code.iter().enumerate() {
        // The game follows the table with a loop that derives these fields from each recipe.
        if instruction.mnemonic.starts_with("stloc")
            && recipe_list_has_entries(&static_fields, &objects)
        {
            registry_end = Some(index);
            break;
        }
        if is_control_flow(instruction) || instruction.mnemonic == "ret" {
            break;
        }
        match instruction.mnemonic {
            "nop" => {}
            "dup" => stack.push(
                stack
                    .last()
                    .cloned()
                    .ok_or_else(|| il_error(instruction, "dup on an empty stack"))?,
            ),
            "pop" => {
                pop(&mut stack, instruction)?;
            }
            "ldstr" => {
                let value = instruction_string(instruction, assembly.userstrings())
                    .ok_or_else(|| il_error(instruction, "unreadable string literal"))?;
                stack.push(Value::String(value));
            }
            opcode if opcode.starts_with("ldc.") => {
                let value = instruction_value(instruction, assembly.userstrings())
                    .and_then(|value| match value {
                        StaticValue::Number(number) => Some(number),
                        StaticValue::String(_) => None,
                    })
                    .ok_or_else(|| il_error(instruction, "unreadable number literal"))?;
                stack.push(Value::Number(value));
            }
            "ldsfld" => {
                let (owner, name) = reader
                    .field(&instruction.operand)
                    .ok_or_else(|| il_error(instruction, "unreadable static field"))?;
                let value = static_fields
                    .get(&(owner, name))
                    .cloned()
                    .ok_or_else(|| il_error(instruction, "unknown static field value"))?;
                stack.push(value);
            }
            "stsfld" => {
                let (owner, name) = reader
                    .field(&instruction.operand)
                    .ok_or_else(|| il_error(instruction, "unreadable static field"))?;
                let value = pop(&mut stack, instruction)?;
                static_fields.insert((owner, name), value);
            }
            "newobj" => {
                let (owner, name) = reader
                    .callee(&instruction.operand)
                    .ok_or_else(|| il_error(instruction, "unreadable constructor"))?;
                let object = match owner.as_str() {
                    "Recipe" if name == ".ctor" => Object {
                        kind: Some(ObjectKind::Recipe),
                        ..Default::default()
                    },
                    "RecipeResult" if name == ".ctor" => Object {
                        kind: Some(ObjectKind::Result),
                        fields: HashMap::from([
                            ("amount".into(), Value::Number(1.0)),
                            ("resultCondition".into(), Value::Number(1.0)),
                        ]),
                        ..Default::default()
                    },
                    "RecipeItem" if name == ".ctor" => {
                        let minimum_condition = pop_number(&mut stack, instruction)?;
                        Object {
                            kind: Some(ObjectKind::Ingredient),
                            fields: HashMap::from([
                                ("minimumCondition".into(), Value::Number(minimum_condition)),
                                ("destroyItem".into(), Value::Number(1.0)),
                            ]),
                            ..Default::default()
                        }
                    }
                    "CraftingQuality" if name == ".ctor" => {
                        let amount = match stack.last() {
                            Some(Value::Number(_)) => pop_number(&mut stack, instruction)?,
                            _ => 1.0,
                        };
                        let id = pop_string(&mut stack, instruction)?;
                        Object {
                            kind: Some(ObjectKind::Quality),
                            fields: HashMap::from([
                                ("id".into(), Value::String(id)),
                                ("amount".into(), Value::Number(amount)),
                            ]),
                            ..Default::default()
                        }
                    }
                    _ if (owner.starts_with("System.Collections.Generic.List")
                        || owner.starts_with("List`1"))
                        && name == ".ctor" =>
                    {
                        Object {
                            kind: Some(ObjectKind::List),
                            ..Default::default()
                        }
                    }
                    _ => {
                        return Err(il_error(
                            instruction,
                            &format!("unsupported constructor {owner}::{name}"),
                        ));
                    }
                };
                stack.push(Value::Object(insert_object(&mut objects, object)));
            }
            "call" | "callvirt" => {
                let (owner, name) = reader
                    .callee(&instruction.operand)
                    .ok_or_else(|| il_error(instruction, "unreadable method call"))?;
                if owner == "CraftingQuality" && name == "op_Implicit" {
                    let id = pop_string(&mut stack, instruction)?;
                    let quality = Object {
                        kind: Some(ObjectKind::Quality),
                        fields: HashMap::from([
                            ("id".into(), Value::String(id)),
                            ("amount".into(), Value::Number(1.0)),
                        ]),
                        ..Default::default()
                    };
                    stack.push(Value::Object(insert_object(&mut objects, quality)));
                } else if name == "Add" {
                    let value = pop(&mut stack, instruction)?;
                    let Value::Object(list) = pop(&mut stack, instruction)? else {
                        return Err(il_error(instruction, "list Add receiver is not an object"));
                    };
                    let Some(target) = objects.get_mut(list) else {
                        return Err(il_error(instruction, "list Add receiver is missing"));
                    };
                    if target.kind != Some(ObjectKind::List) {
                        return Err(il_error(instruction, "list Add receiver is not a list"));
                    }
                    target.values.push(value);
                } else {
                    return Err(il_error(
                        instruction,
                        &format!("unsupported method call {owner}::{name}"),
                    ));
                }
            }
            "stfld" => {
                let value = pop(&mut stack, instruction)?;
                let Value::Object(target) = pop(&mut stack, instruction)? else {
                    return Err(il_error(instruction, "field receiver is not an object"));
                };
                let (_, field) = reader
                    .field(&instruction.operand)
                    .ok_or_else(|| il_error(instruction, "unreadable instance field"))?;
                objects
                    .get_mut(target)
                    .ok_or_else(|| il_error(instruction, "field receiver is missing"))?
                    .fields
                    .insert(field, value);
            }
            opcode => {
                return Err(il_error(
                    instruction,
                    &format!("unsupported instruction {opcode}"),
                ));
            }
        }
    }
    let registry_end =
        registry_end.ok_or("Cannot find the end of Recipes.SetUpRecipes registry")?;
    for field in ["index", "specific", "ignoredId"] {
        if !code[registry_end..].iter().any(|instruction| {
            reader
                .field(&instruction.operand)
                .is_some_and(|(owner, name)| {
                    (owner == "Recipe" || owner == "RecipeItem") && name == field
                })
        }) {
            return Err(format!(
                "Recipes.SetUpRecipes no longer contains the expected {field} post-processing"
            ));
        }
    }

    let Value::Object(list) = static_fields
        .get(&("Recipes".into(), "recipes".into()))
        .ok_or("Recipes.SetUpRecipes did not initialize Recipes.recipes")?
    else {
        return Err("Recipes.SetUpRecipes initialized Recipes.recipes with a non-list".into());
    };
    let recipes = objects
        .get(*list)
        .filter(|object| object.kind == Some(ObjectKind::List))
        .ok_or("Recipes.SetUpRecipes initialized Recipes.recipes with an invalid list")?;
    let parsed = recipes
        .values
        .iter()
        .enumerate()
        .map(|(index, value)| parse_recipe(index, value, &objects))
        .collect::<Result<Vec<_>, _>>()?;
    if parsed.is_empty() {
        return Err("Recipes.SetUpRecipes contained no recipe definitions".into());
    }
    Ok(parsed)
}

fn recipe_list_has_entries(
    static_fields: &HashMap<(String, String), Value>,
    objects: &[Object],
) -> bool {
    let Some(Value::Object(list)) = static_fields.get(&("Recipes".into(), "recipes".into())) else {
        return false;
    };
    objects
        .get(*list)
        .is_some_and(|object| object.kind == Some(ObjectKind::List) && !object.values.is_empty())
}

fn is_control_flow(instruction: &Instruction) -> bool {
    !instruction.branch_targets.is_empty()
        || matches!(
            instruction.mnemonic,
            "br" | "br.s"
                | "brfalse"
                | "brfalse.s"
                | "brtrue"
                | "brtrue.s"
                | "beq"
                | "beq.s"
                | "bge"
                | "bge.s"
                | "bge.un"
                | "bge.un.s"
                | "bgt"
                | "bgt.s"
                | "bgt.un"
                | "bgt.un.s"
                | "ble"
                | "ble.s"
                | "ble.un"
                | "ble.un.s"
                | "blt"
                | "blt.s"
                | "blt.un"
                | "blt.un.s"
                | "bne.un"
                | "bne.un.s"
                | "leave"
                | "leave.s"
                | "switch"
        )
}

fn insert_object(objects: &mut Vec<Object>, object: Object) -> usize {
    let id = objects.len();
    objects.push(object);
    id
}

fn pop(stack: &mut Vec<Value>, instruction: &Instruction) -> Result<Value, String> {
    stack
        .pop()
        .ok_or_else(|| il_error(instruction, "stack underflow"))
}

fn pop_number(stack: &mut Vec<Value>, instruction: &Instruction) -> Result<f32, String> {
    match pop(stack, instruction)? {
        Value::Number(value) if value.is_finite() => Ok(value),
        _ => Err(il_error(instruction, "expected a finite number")),
    }
}

fn pop_string(stack: &mut Vec<Value>, instruction: &Instruction) -> Result<String, String> {
    match pop(stack, instruction)? {
        Value::String(value) => Ok(value),
        _ => Err(il_error(instruction, "expected a string")),
    }
}

fn il_error(instruction: &Instruction, message: &str) -> String {
    format!(
        "Cannot read Recipes.SetUpRecipes at IL_{:04x}: {message}",
        instruction.rva
    )
}

fn field<'a>(object: &'a Object, key: &str) -> Option<&'a Value> {
    object.fields.get(key)
}

fn object<'a>(
    value: Option<&Value>,
    objects: &'a [Object],
    kind: ObjectKind,
) -> Option<&'a Object> {
    let Value::Object(index) = value? else {
        return None;
    };
    objects
        .get(*index)
        .filter(|object| object.kind == Some(kind))
}

fn number(object: &Object, key: &str, default: f32) -> f32 {
    match field(object, key) {
        Some(Value::Number(value)) => *value,
        _ => default,
    }
}

fn string(object: &Object, key: &str) -> Option<String> {
    match field(object, key) {
        Some(Value::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn boolean(object: &Object, key: &str, default: bool) -> bool {
    match field(object, key) {
        Some(Value::Number(value)) => *value != 0.0,
        _ => default,
    }
}

fn parse_recipe(index: usize, value: &Value, objects: &[Object]) -> Result<CraftingRecipe, String> {
    let Some(recipe) = object(Some(value), objects, ObjectKind::Recipe) else {
        return Err(format!("Recipe #{index} has an invalid definition"));
    };
    let result = object(field(recipe, "result"), objects, ObjectKind::Result)
        .ok_or_else(|| format!("Recipe #{index} has no result"))?;
    let result_id =
        string(result, "id").ok_or_else(|| format!("Recipe #{index} has no result ID"))?;
    let item_list = object(field(recipe, "items"), objects, ObjectKind::List)
        .ok_or_else(|| format!("Recipe #{index} has no ingredient list"))?;
    let is_repair = boolean(recipe, "isRepair", false);
    let ingredients = item_list
        .values
        .iter()
        .map(|value| parse_ingredient(index, value, objects, is_repair, &result_id))
        .collect::<Result<Vec<_>, _>>()?;
    let category = match number(recipe, "category", 0.0) as i32 {
        0 => RecipeCategory::Materials,
        1 => RecipeCategory::Tools,
        2 => RecipeCategory::Medicine,
        3 => RecipeCategory::Utilities,
        4 => RecipeCategory::Food,
        category => return Err(format!("Recipe #{index} has unknown category {category}")),
    };

    Ok(CraftingRecipe {
        index,
        intelligence: number(recipe, "INT", 0.0) as i32,
        category,
        special_known: boolean(recipe, "specialKnown", false),
        is_repair,
        result_id,
        result_is_liquid: boolean(result, "isLiquid", false),
        result_amount: number(result, "amount", 1.0) as i32,
        result_condition: number(result, "resultCondition", 1.0),
        dont_drain_result_liquid: boolean(result, "dontDrainResultLiquid", false),
        ingredients,
    })
}

fn parse_ingredient(
    recipe_index: usize,
    value: &Value,
    objects: &[Object],
    is_repair: bool,
    result_id: &str,
) -> Result<CraftingIngredient, String> {
    let Some(ingredient) = object(Some(value), objects, ObjectKind::Ingredient) else {
        return Err(format!("Recipe #{recipe_index} has an invalid ingredient"));
    };
    let specific_id = string(ingredient, "specificId");
    let specific = boolean(ingredient, "specific", false) || specific_id.is_some();
    let quality = object(field(ingredient, "quality"), objects, ObjectKind::Quality);
    let quality_id = quality.and_then(|quality| string(quality, "id"));
    if (specific && specific_id.is_none()) || (!specific && quality_id.is_none()) {
        return Err(format!(
            "Recipe #{recipe_index} has an ingredient without a matching rule"
        ));
    }
    Ok(CraftingIngredient {
        is_liquid: boolean(ingredient, "isLiquid", false),
        specific_id: specific.then_some(specific_id).flatten(),
        quality_id,
        quality_amount: quality.map_or(0.0, |quality| number(quality, "amount", 1.0)),
        minimum_condition: number(ingredient, "minimumCondition", 0.9),
        destroy_item: boolean(ingredient, "destroyItem", true),
        ignored_id: if is_repair {
            None
        } else {
            string(ingredient, "ignoredId")
                .filter(|id| !id.is_empty())
                .or_else(|| Some(result_id.to_owned()))
        },
        no_max_condition: boolean(ingredient, "noMaxCondition", false),
    })
}
