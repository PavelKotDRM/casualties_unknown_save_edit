//! Bounded readers for Unity 2022 serialized player and item prefabs.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use dotscope::CilObject;
use dotscope::metadata::signatures::TypeSignature;
use dotscope::metadata::tables::FieldAttributes;
use unity_rs_core::studio::{Studio, StudioObject};

use super::ItemStats;
use super::metadata::Metadata;
use super::runtime::intern;
use crate::schema::{HoldSlotDef, LimbDef};

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

pub(super) fn moodle_palette(data_dir: &Path) -> Result<Vec<[u8; 3]>, String> {
    use unity_rs_core::scene::{ObjectReference, resolve_object_reference};
    let studio = Studio::open(data_dir).map_err(|error| error.to_string())?;
    let manager = studio
        .objects()
        .find(|object| object.class_id() == 114 && object.name() == Some("MoodleManager"))
        .ok_or("Missing MoodleManager status palette")?;
    let bytes = manager
        .read_raw(1024 * 1024)
        .map_err(|error| error.to_string())?;
    let mut cursor = Cursor {
        bytes: &bytes,
        offset: mono_behaviour_data_offset(&bytes).ok_or("Invalid MoodleManager header")?,
    };
    cursor
        .reference()
        .ok_or("Invalid MoodleManager transform reference")?;
    let count = cursor
        .i32()
        .and_then(|count| usize::try_from(count).ok())
        .filter(|count| (1..=256).contains(count))
        .ok_or("Invalid MoodleManager background palette size")?;
    let mut palette = Vec::with_capacity(count);
    for _ in 0..count {
        let (file_id, path_id) = cursor
            .reference()
            .ok_or("Truncated MoodleManager palette")?;
        let target = resolve_object_reference(
            studio.collection(),
            manager.file_index(),
            ObjectReference { file_id, path_id },
        )
        .map_err(|error| error.to_string())?
        .ok_or("Null status background sprite")?;
        let sprite = studio
            .object_by_index(target.file_index, target.object_index)
            .ok_or("Missing status background sprite")?;
        let image = sprite
            .decode_sprite(Default::default(), Default::default())
            .map_err(|error| format!("Cannot decode status palette: {error}"))?;
        let mut totals = [0_u64; 4];
        for pixel in image.pixels.as_chunks::<4>().0 {
            for channel in 0..3 {
                totals[channel] += u64::from(pixel[channel]) * u64::from(pixel[3]);
            }
            totals[3] += u64::from(pixel[3]);
        }
        if totals[3] == 0 {
            return Err("Status background sprite is fully transparent".into());
        }
        palette.push([
            (totals[0] / totals[3]) as u8,
            (totals[1] / totals[3]) as u8,
            (totals[2] / totals[3]) as u8,
        ]);
    }
    Ok(palette)
}

fn enum_constant(assembly: &CilObject, owner: &str, name: &str) -> Option<i64> {
    assembly.types().iter().find_map(|entry| {
        let ty = entry.value();
        (ty.name == owner)
            .then(|| {
                ty.fields().find_map(|field| {
                    (field.name == name)
                        .then(|| field.default.get()?.as_i64())
                        .flatten()
                })
            })
            .flatten()
    })
}

pub(super) fn ammunition(
    data_dir: &Path,
    assembly: &CilObject,
) -> Result<super::ammunition::AmmunitionData, String> {
    use super::ammunition::{AmmoDef, AmmunitionData, ChamberValues, GunDef};
    use std::collections::BTreeMap;

    let constant = |owner, name| {
        enum_constant(assembly, owner, name)
            .ok_or_else(|| format!("Game ammunition enum {owner}.{name} is unavailable"))
    };
    let magazine_kind = constant("AmmoItemType", "Magazine")?;
    let round_kind = constant("AmmoItemType", "Round")?;
    let magazine_feed = constant("FeedType", "Mag")?;
    let direct_feed = constant("FeedType", "Direct")?;
    let chamber = ChamberValues {
        round: constant("RoundInChamber", "Round")?,
        casing: constant("RoundInChamber", "Casing")?,
        empty: constant("RoundInChamber", "None")?,
    };
    let studio = Studio::open(data_dir).map_err(|error| error.to_string())?;
    let ids = resource_item_ids(&studio)?;
    let reader = super::metadata::Reader::new(assembly);
    let magazine_resources: HashSet<_> = reader
        .method("AmmoScript", "AmmoTypeToMagazine")
        .ok_or("Missing AmmoScript.AmmoTypeToMagazine")?
        .instructions()
        .filter(|instruction| instruction.mnemonic == "ldstr")
        .filter_map(|instruction| super::instruction_string(instruction, assembly.userstrings()))
        .collect();
    let mut items = BTreeMap::new();
    let mut guns = BTreeMap::new();
    for owner in ["AmmoScript", "GunScript"] {
        let fields = prefab_fields(assembly, owner)?;
        let saved = saved_fields(assembly, owner);
        let constructor = super::metadata::constructor_defaults(&reader, owner);
        for object in studio
            .objects()
            .filter(|o| o.class_id() == 114 && o.name() == Some(owner))
        {
            let bytes = object.read_raw(1024 * 1024).map_err(|e| e.to_string())?;
            let mut cursor = Cursor {
                bytes: &bytes,
                offset: 0,
            };
            let (file, path) = cursor
                .reference()
                .ok_or("Truncated ammunition component reference")?;
            let Some(id) = (file == 0)
                .then(|| ids.get(&(object.file_index(), path)))
                .flatten()
            else {
                continue;
            };
            cursor.offset =
                mono_behaviour_data_offset(&bytes).ok_or("Invalid ammunition component header")?;
            let mut values: serde_json::Map<_, _> = constructor.clone().into_iter().collect();
            for (name, ty) in &fields {
                let Some(value) = cursor.value(assembly, ty) else {
                    break;
                };
                values.insert(name.clone(), value);
            }
            let integer = |key| {
                values
                    .get(key)
                    .and_then(serde_json::Value::as_i64)
                    .ok_or_else(|| format!("Missing ammunition field {owner}.{key} for {id}"))
            };
            let ammo_type = integer("ammoType")?;
            let kind = if owner == "AmmoScript" {
                integer("itemType")?
            } else {
                integer("feedType")?
            };
            let defaults = values
                .into_iter()
                .filter(|(key, _)| saved.contains(key))
                .collect();
            if owner == "AmmoScript" {
                if kind != magazine_kind && kind != round_kind {
                    return Err(format!("Unsupported ammunition kind for {id}"));
                }
                let def = AmmoDef {
                    ammo_type,
                    magazine: kind == magazine_kind,
                    insertable: kind == magazine_kind && magazine_resources.contains(id),
                    defaults,
                };
                if items.get(id).is_some_and(|previous| previous != &def) {
                    return Err(format!("Conflicting ammunition prefabs for {id}"));
                }
                items.insert(id.clone(), def);
            } else {
                if kind != magazine_feed && kind != direct_feed {
                    return Err(format!("Unsupported weapon feed type for {id}"));
                }
                let def = GunDef {
                    ammo_type,
                    magazine_fed: kind == magazine_feed,
                    defaults,
                };
                if guns.get(id).is_some_and(|previous| previous != &def) {
                    return Err(format!("Conflicting weapon prefabs for {id}"));
                }
                guns.insert(id.clone(), def);
            }
        }
    }
    Ok(AmmunitionData {
        items,
        guns,
        chamber,
    })
}

fn prefab_fields(
    assembly: &CilObject,
    owner: &str,
) -> Result<Vec<(String, TypeSignature)>, String> {
    assembly
        .types()
        .iter()
        .find_map(|entry| {
            let ty = entry.value();
            (ty.name == owner).then(|| {
                ty.fields()
                    .filter(|field| {
                        !field.flags.is_static()
                            && !field.flags.is_not_serialized()
                            && field.flags.access() == FieldAttributes::PUBLIC
                    })
                    .map(|field| (field.name.clone(), field.signature.base.clone()))
                    .collect()
            })
        })
        .ok_or_else(|| format!("Missing game type {owner}"))
}

fn saved_fields(assembly: &CilObject, owner: &str) -> HashSet<String> {
    assembly
        .types()
        .iter()
        .filter(|entry| entry.value().name == owner)
        .flat_map(|entry| {
            entry
                .value()
                .fields()
                .filter(|field| {
                    field.custom_attributes.iter().any(|(_, attribute)| {
                        matches!(&attribute.constructor,
                    dotscope::metadata::typesystem::CilTypeReference::MemberRef(member)
                    if member.declaredby.name().as_deref() == Some("JsonPropertyAttribute"))
                    })
                })
                .map(|field| field.name.clone())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn resource_item_ids(studio: &Studio) -> Result<HashMap<(usize, i64), String>, String> {
    let mut ids = HashMap::new();
    for object in studio
        .objects()
        .filter(|o| o.class_id() == 114 && o.name() == Some("Item"))
    {
        let bytes = object.read_raw(1024 * 1024).map_err(|e| e.to_string())?;
        let mut cursor = Cursor {
            bytes: &bytes,
            offset: 0,
        };
        let reference = cursor
            .reference()
            .ok_or("Truncated resource item reference")?;
        cursor.offset = mono_behaviour_data_offset(&bytes).ok_or("Invalid resource item header")?;
        let id = cursor.string().ok_or("Invalid resource item ID")?;
        let resource = studio
            .object(object.file_index(), reference.1)
            .is_some_and(|object| {
                object.container().is_some_and(|resource| {
                    let name = resource.rsplit(['/', '\\']).next().unwrap_or(resource);
                    name.strip_suffix(".prefab")
                        .unwrap_or(name)
                        .eq_ignore_ascii_case(&id)
                })
            });
        if reference.0 == 0 && resource {
            ids.insert((object.file_index(), reference.1), id);
        }
    }
    Ok(ids)
}

#[derive(Debug, PartialEq)]
pub(super) struct BatteryDefaults {
    pub state: serde_json::Value,
    pub max_allowed_charge: f32,
}

pub(super) fn batteries(
    data_dir: &Path,
    assembly: &CilObject,
) -> Result<std::collections::BTreeMap<String, BatteryDefaults>, String> {
    let studio = Studio::open(data_dir).map_err(|error| error.to_string())?;
    let ids = resource_item_ids(&studio)?;
    let reader = super::metadata::Reader::new(assembly);
    let owner = "BatteryItem";
    let fields = prefab_fields(assembly, owner)?;
    let saved = saved_fields(assembly, owner);
    let constructor = super::metadata::constructor_defaults(&reader, owner);
    let awake = reader
        .method(owner, "Awake")
        .ok_or("Missing BatteryItem.Awake")?;
    let mut batteries = std::collections::BTreeMap::new();
    for object in studio
        .objects()
        .filter(|o| o.class_id() == 114 && o.name() == Some(owner))
    {
        let bytes = object.read_raw(1024 * 1024).map_err(|e| e.to_string())?;
        let mut cursor = Cursor {
            bytes: &bytes,
            offset: 0,
        };
        let (file, path) = cursor
            .reference()
            .ok_or("Truncated battery component reference")?;
        let Some(id) = (file == 0)
            .then(|| ids.get(&(object.file_index(), path)))
            .flatten()
        else {
            continue;
        };
        cursor.offset =
            mono_behaviour_data_offset(&bytes).ok_or("Invalid battery component header")?;
        let mut values: serde_json::Map<_, _> = constructor.clone().into_iter().collect();
        for (name, ty) in &fields {
            let value = cursor
                .value(assembly, ty)
                .ok_or_else(|| format!("Cannot decode battery field {name} for {id}"))?;
            values.insert(name.clone(), value);
        }
        initialize_battery(&reader, &awake, &mut values)
            .ok_or_else(|| format!("Cannot evaluate BatteryItem.Awake for {id}"))?;
        let max_allowed_charge = values
            .get("maxAllowedCharge")
            .and_then(serde_json::Value::as_f64)
            .filter(|charge| charge.is_finite() && *charge > 0.0)
            .ok_or_else(|| format!("Invalid battery capacity for {id}"))?
            as f32;
        let defaults: serde_json::Map<_, _> = values
            .into_iter()
            .filter(|(key, _)| saved.contains(key))
            .collect();
        let defaults = BatteryDefaults {
            state: serde_json::Value::Object(defaults),
            max_allowed_charge,
        };
        if batteries
            .get(id)
            .is_some_and(|previous| previous != &defaults)
        {
            return Err(format!("Conflicting battery prefabs for {id}"));
        }
        batteries.insert(id.clone(), defaults);
    }
    Ok(batteries)
}

fn initialize_battery(
    reader: &super::metadata::Reader<'_>,
    method: &dotscope::metadata::method::MethodRc,
    values: &mut serde_json::Map<String, serde_json::Value>,
) -> Option<()> {
    use serde_json::{Value, json};
    let code: Vec<_> = method.instructions().collect();
    // The initial GetComponent assignment is a runtime reference, not saved state.
    let mut pc = code.windows(2).position(|pair| {
        pair[0].mnemonic == "ldarg.0"
            && pair[1].mnemonic == "ldfld"
            && reader.field(&pair[1].operand) == Some(("BatteryItem".into(), "preset".into()))
    })?;
    let mut stack = Vec::<Value>::new();
    let mut locals = HashMap::new();
    let local = |instruction: &dotscope::assembly::Instruction| {
        instruction
            .mnemonic
            .rsplit('.')
            .next()?
            .parse::<usize>()
            .ok()
    };
    for _ in 0..1024 {
        let instruction = code.get(pc)?;
        pc += 1;
        match instruction.mnemonic {
            "ldarg.0" => stack.push(Value::Null),
            "ldfld" => {
                if !stack.pop()?.is_null() {
                    return None;
                }
                let (owner, name) = reader.field(&instruction.operand)?;
                if owner != "BatteryItem" {
                    return None;
                }
                stack.push(values.get(&name)?.clone());
            }
            "stfld" => {
                let value = stack.pop()?;
                if !stack.pop()?.is_null() {
                    return None;
                }
                let (owner, name) = reader.field(&instruction.operand)?;
                if owner != "BatteryItem" {
                    return None;
                }
                values.insert(name, value);
            }
            op if op.starts_with("ldc.") || op == "ldstr" => {
                stack.push(
                    match super::instruction_value(instruction, reader.assembly.userstrings())? {
                        super::StaticValue::Number(value) => json!(value),
                        super::StaticValue::String(value) => value.into(),
                    },
                );
            }
            op if op.starts_with("stloc.") => {
                locals.insert(local(instruction)?, stack.pop()?);
            }
            op if op.starts_with("ldloc.") => {
                stack.push(locals.get(&local(instruction)?)?.clone());
            }
            "switch" => {
                let index = stack.pop()?.as_f64()?;
                if index < 0.0 || index.fract() != 0.0 {
                    return None;
                }
                let target = instruction.branch_targets.get(index as usize)?;
                pc = code
                    .iter()
                    .position(|instruction| instruction.rva == *target)?;
            }
            "brfalse" | "brfalse.s" | "brtrue" | "brtrue.s" => {
                let value = stack.pop()?;
                let truth = value
                    .as_bool()
                    .or_else(|| value.as_f64().map(|value| value != 0.0))?;
                if truth == instruction.mnemonic.starts_with("brtrue") {
                    let target = instruction.branch_targets.first()?;
                    pc = code
                        .iter()
                        .position(|instruction| instruction.rva == *target)?;
                }
            }
            "br" | "br.s" => {
                let target = instruction.branch_targets.first()?;
                pc = code
                    .iter()
                    .position(|instruction| instruction.rva == *target)?;
            }
            "nop" => {}
            "ret" => return stack.is_empty().then_some(()),
            _ => return None,
        }
    }
    None
}

impl<'a> Cursor<'a> {
    fn skip(&mut self, count: usize) -> Option<()> {
        self.offset = self.offset.checked_add(count)?;
        (self.offset <= self.bytes.len()).then_some(())
    }

    fn i32(&mut self) -> Option<i32> {
        let value = read_i32(self.bytes, self.offset)?;
        self.skip(4)?;
        Some(value)
    }

    fn reference(&mut self) -> Option<(i32, i64)> {
        let file = self.i32()?;
        let value = i64::from_le_bytes(
            self.bytes
                .get(self.offset..self.offset.checked_add(8)?)?
                .try_into()
                .ok()?,
        );
        self.skip(8)?;
        Some((file, value))
    }

    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_bits(self.i32()? as u32))
    }

    fn strings(&mut self) -> Option<Vec<String>> {
        let count = usize::try_from(self.i32()?)
            .ok()
            .filter(|count| *count <= 1024)?;
        (0..count).map(|_| self.string()).collect()
    }

    fn string(&mut self) -> Option<String> {
        let (value, next) = read_aligned_string_at(self.bytes, self.offset)?;
        if next > self.bytes.len() {
            return None;
        }
        self.offset = next;
        Some(value)
    }
}

fn read_i32(bytes: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn read_aligned_string_at(bytes: &[u8], offset: usize) -> Option<(String, usize)> {
    let length = usize::try_from(read_i32(bytes, offset)?)
        .ok()
        .filter(|length| *length <= 16 * 1024 * 1024)?;
    let start = offset.checked_add(4)?;
    let end = start.checked_add(length)?;
    let next = end.checked_add(3)? & !3;
    if next > bytes.len() {
        return None;
    }
    Some((
        std::str::from_utf8(bytes.get(start..end)?).ok()?.to_owned(),
        next,
    ))
}

fn mono_behaviour_data_offset(bytes: &[u8]) -> Option<usize> {
    read_aligned_string_at(bytes, 28).map(|(_, next)| next)
}

#[derive(Debug, Clone, PartialEq)]
struct ContainerMetadata {
    capacity: f32,
    max_item_weight: f32,
    encumbrance: f32,
    tags: HashSet<String>,
}

pub(super) fn containers(
    data_dir: &Path,
    assembly: &CilObject,
    items: &mut HashMap<String, ItemStats>,
) -> Result<(), String> {
    let reader = super::metadata::Reader::new(assembly);
    let mut initializers = HashMap::new();
    if let Some(method) = reader.method("Container", "Awake") {
        let instructions: Vec<_> = method.instructions().collect();
        for (index, instruction) in instructions.iter().enumerate() {
            if instruction.mnemonic != "stfld" {
                continue;
            }
            let Some((owner, field)) = reader.field(&instruction.operand) else {
                continue;
            };
            if owner != "Container" || !matches!(field.as_str(), "maxWeight" | "maxWeightPerItem") {
                continue;
            }
            let expression = index
                .checked_sub(4)
                .and_then(|start| {
                    super::program::ScalarProgram::read(assembly, &instructions[start..=index])
                })
                .ok_or_else(|| format!("Unsupported container initializer for {field}"))?;
            if initializers.insert(field.clone(), expression).is_some() {
                return Err(format!("Multiple container initializers for {field}"));
            }
        }
    }
    let studio = Studio::open(data_dir).map_err(|error| {
        format!(
            "Cannot read Unity assets in {}: {error}",
            data_dir.display()
        )
    })?;
    let version = studio
        .files()
        .next()
        .map(|file| file.unity_version().to_owned())
        .ok_or("Unity data directory contains no serialized files")?;
    if !version.starts_with("2022.") {
        return Err(format!(
            "Unsupported Unity serialization version {version:?}"
        ));
    }
    let mut containers = HashMap::new();
    for object in studio
        .objects()
        .filter(|object| object.class_id() == 114 && object.name() == Some("Container"))
    {
        let bytes = object.read_raw(4096).map_err(|error| error.to_string())?;
        let mut cursor = Cursor {
            bytes: &bytes,
            offset: 0,
        };
        let (file, path) = cursor
            .reference()
            .ok_or("Truncated container GameObject reference")?;
        if file != 0 {
            continue;
        }
        cursor.offset =
            mono_behaviour_data_offset(&bytes).ok_or("Invalid Container MonoBehaviour header")?;
        let valid = |value: f32| (value.is_finite() && value >= 0.0).then_some(value);
        let mut capacity = cursor
            .f32()
            .and_then(valid)
            .ok_or("Invalid container capacity")?;
        let mut max_item_weight = cursor
            .f32()
            .and_then(valid)
            .ok_or("Invalid container maximum item weight")?;
        let encumbrance = cursor
            .f32()
            .and_then(valid)
            .ok_or("Invalid container encumbrance multiplier")?;
        // Awake applies the game's own rounding allowance to prefab limits.
        let fields = HashMap::from([
            ("maxWeight".into(), capacity),
            ("maxWeightPerItem".into(), max_item_weight),
        ]);
        for (field, value) in [
            ("maxWeight", &mut capacity),
            ("maxWeightPerItem", &mut max_item_weight),
        ] {
            if let Some(initializer) = initializers.get(field) {
                *value = initializer
                    .evaluate_fields(None, &fields)
                    .and_then(valid)
                    .ok_or_else(|| format!("Cannot evaluate container initializer for {field}"))?;
            }
        }
        cursor.skip(4).ok_or("Truncated container fields")?;
        let metadata = ContainerMetadata {
            capacity,
            max_item_weight,
            encumbrance,
            tags: cursor
                .strings()
                .ok_or("Invalid container tag restrictions")?
                .into_iter()
                .collect(),
        };
        let game_object = studio
            .object(object.file_index(), path)
            .ok_or("Missing container GameObject")?;
        let bytes = game_object
            .read_raw(1024 * 1024)
            .map_err(|error| error.to_string())?;
        let mut cursor = Cursor {
            bytes: &bytes,
            offset: 0,
        };
        let count = usize::try_from(cursor.i32().ok_or("Missing component count")?)
            .ok()
            .filter(|count| *count <= 1024)
            .ok_or("Invalid component count")?;
        for _ in 0..count {
            let (file, path) = cursor.reference().ok_or("Truncated component reference")?;
            if file != 0 {
                continue;
            }
            let Some(item) = studio.object(object.file_index(), path) else {
                continue;
            };
            if item.class_id() != 114 || item.name() != Some("Item") {
                continue;
            }
            let bytes = item
                .read_raw(1024 * 1024)
                .map_err(|error| error.to_string())?;
            let offset =
                mono_behaviour_data_offset(&bytes).ok_or("Invalid Item MonoBehaviour header")?;
            let id = Cursor {
                bytes: &bytes,
                offset,
            }
            .string()
            .ok_or("Invalid container item ID")?;
            if containers
                .get(&id)
                .is_some_and(|previous| previous != &metadata)
            {
                return Err(format!(
                    "Container metadata for item {id:?} has conflicting values"
                ));
            }
            containers.insert(id, metadata.clone());
        }
    }
    for (id, metadata) in containers {
        if let Some(item) = items.get_mut(&id) {
            item.container_capacity = Some(metadata.capacity);
            item.container_max_item_weight = Some(metadata.max_item_weight);
            item.container_encumbrance = Some(metadata.encumbrance);
            item.container_tag_restriction = Some(metadata.tags);
        }
    }
    Ok(())
}

impl<'a> Cursor<'a> {
    fn align(&mut self) -> Option<()> {
        let aligned = self.offset.checked_add(3)? & !3;
        self.skip(aligned - self.offset)
    }

    fn value(&mut self, assembly: &CilObject, ty: &TypeSignature) -> Option<serde_json::Value> {
        use serde_json::json;
        Some(match ty {
            TypeSignature::Boolean => {
                let value = *self.bytes.get(self.offset)? != 0;
                self.skip(1)?;
                self.align()?;
                json!(value)
            }
            TypeSignature::I4 => json!(self.i32()?),
            TypeSignature::R4 => json!(f32::from_bits(self.i32()? as u32)),
            TypeSignature::String => json!(self.string()?),
            TypeSignature::SzArray(array) => {
                let count = usize::try_from(self.i32()?)
                    .ok()
                    .filter(|count| *count <= 4096)?;
                let mut values = Vec::with_capacity(count);
                for _ in 0..count {
                    values.push(self.value(assembly, &array.base)?);
                }
                json!(values)
            }
            TypeSignature::GenericInst(base, arguments) => {
                let (TypeSignature::Class(token) | TypeSignature::ValueType(token)) = base.as_ref()
                else {
                    return None;
                };
                match assembly.types().get(token)?.name.as_str() {
                    // Unity does not serialize nullable values.
                    "Nullable`1" => serde_json::Value::Null,
                    "List`1" => {
                        let [item] = arguments.as_slice() else {
                            return None;
                        };
                        let count = usize::try_from(self.i32()?)
                            .ok()
                            .filter(|count| *count <= 4096)?;
                        let mut values = Vec::with_capacity(count);
                        for _ in 0..count {
                            values.push(self.value(assembly, item)?);
                        }
                        json!(values)
                    }
                    _ => return None,
                }
            }
            TypeSignature::Class(token) | TypeSignature::ValueType(token) => {
                let definition = assembly.types().get(token);
                if definition
                    .as_ref()
                    .is_some_and(|ty| ty.fields().any(|field| field.name == "value__"))
                {
                    return Some(json!(self.i32()?));
                }
                let name = definition.as_ref().map(|ty| ty.name.as_str()).unwrap_or("");
                match name {
                    "Vector2" => {
                        self.skip(8)?;
                        serde_json::Value::Null
                    }
                    "Vector3" => {
                        self.skip(12)?;
                        serde_json::Value::Null
                    }
                    // Unity 2022.3 layout: 8 RGBA keys, 16 u16 key times, then
                    // mode, colour space and both key counts as single bytes.
                    "Gradient" => {
                        self.skip(164)?;
                        serde_json::Value::Null
                    }
                    "AnimationCurve" => {
                        let count = usize::try_from(self.i32()?)
                            .ok()
                            .filter(|count| *count <= 4096)?;
                        self.skip(count.checked_mul(28)?.checked_add(12)?)?;
                        serde_json::Value::Null
                    }
                    // Game types without [Serializable] that are not Unity
                    // objects are not serialized at all.
                    _ if definition.as_ref().is_some_and(|ty| {
                        !ty.is_typeref()
                            && !ty.flags.is_serializable()
                            && !ty.base_chain().any(|base| base.namespace == "UnityEngine")
                    }) =>
                    {
                        serde_json::Value::Null
                    }
                    _ if matches!(ty, TypeSignature::Class(_)) => {
                        let (file, path) = self.reference()?;
                        json!([file, path])
                    }
                    _ => return None,
                }
            }
            _ => return None,
        })
    }
}

fn object_name(object: StudioObject<'_>) -> Option<String> {
    let bytes = object.read_raw(1024 * 1024).ok()?;
    let mut cursor = Cursor {
        bytes: &bytes,
        offset: 0,
    };
    let count = usize::try_from(cursor.i32()?)
        .ok()
        .filter(|count| *count <= 4096)?;
    cursor.skip(count.checked_mul(12)?.checked_add(4)?)?;
    cursor.string()
}

fn component_name(studio: &Studio, file: usize, path: i64) -> Option<String> {
    let component = studio.object(file, path)?;
    let bytes = component.read_raw(1024 * 1024).ok()?;
    let mut cursor = Cursor {
        bytes: &bytes,
        offset: 0,
    };
    let (other, path) = cursor.reference()?;
    if other != 0 {
        return None;
    }
    object_name(studio.object(file, path)?)
}

pub(super) fn player(
    data_dir: &Path,
    assembly: &CilObject,
    metadata: &mut Metadata,
    locale: Option<&crate::locale::Locale>,
) -> Result<(), String> {
    let studio = Studio::open(data_dir).map_err(|error| error.to_string())?;
    let fields = assembly
        .types()
        .iter()
        .find_map(|entry| {
            let ty = entry.value();
            (ty.name == "Body").then(|| {
                ty.fields()
                    .filter(|field| {
                        !field.flags.is_static()
                            && !field.flags.is_not_serialized()
                            && field.flags.access() == FieldAttributes::PUBLIC
                    })
                    .map(|field| (field.name.clone(), field.signature.base.clone()))
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default();
    let constructor_defaults = metadata.body_defaults.clone();
    let mut body_defaults_from_prefab = false;
    for object in studio
        .objects()
        .filter(|object| object.class_id() == 114 && object.name() == Some("Body"))
    {
        let bytes = object
            .read_raw(4 * 1024 * 1024)
            .map_err(|error| error.to_string())?;
        let Some(offset) = mono_behaviour_data_offset(&bytes) else {
            continue;
        };
        let mut cursor = Cursor {
            bytes: &bytes,
            offset,
        };
        let mut limbs = Vec::new();
        let mut slots = Vec::new();
        let mut defaults = constructor_defaults.clone();
        let mut character = None;
        let mut height_multiplier = None;
        for (field, ty) in &fields {
            let Some(value) = cursor.value(assembly, ty) else {
                break;
            };
            match field.as_str() {
                "charType" => character = value.as_u64().and_then(|c| usize::try_from(c).ok()),
                "heightMultiplier" => {
                    height_multiplier = value
                        .as_f64()
                        .filter(|value| value.is_finite() && *value > 0.0)
                }
                _ => {}
            }
            if field == "limbs" || field == "slots" {
                let Some(values) = value.as_array() else {
                    break;
                };
                let mut names = Vec::new();
                for reference in values {
                    let Some(file) = reference[0].as_i64() else {
                        break;
                    };
                    let Some(path) = reference[1].as_i64() else {
                        break;
                    };
                    if file != 0 {
                        break;
                    }
                    let Some(name) = component_name(&studio, object.file_index(), path) else {
                        break;
                    };
                    names.push(name);
                }

                if names.len() != values.len() {
                    break;
                }
                if field == "limbs" {
                    limbs = names;
                } else {
                    slots = names;
                }
            } else if defaults.contains_key(field) && !value.is_null() {
                defaults.insert(field.clone(), value);
            }
        }
        if limbs.is_empty() || slots.is_empty() {
            continue;
        }
        let limb_defs: Vec<_> = limbs
            .iter()
            .map(|name| LimbDef {
                object: intern(name),
                name: intern(locale.and_then(|locale| locale.other(name)).unwrap_or(name)),
            })
            .collect();
        let slot_defs: Vec<_> = slots
            .iter()
            .enumerate()
            .map(|(index, _)| {
                // Body.slots uses this order; its GameObjects are all named InvSlot.
                let (name, key) = match index {
                    0 => ("Right hand", "HandF"),
                    1 => ("Left hand", "HandB"),
                    2 => ("Mouth", "mouth"),
                    3 => ("Upper back", "upperback"),
                    4 => ("Middle back", "middleback"),
                    5 => ("Lower back", "lowerback"),
                    _ => (intern(format!("Unknown slot {index}")), ""),
                };
                HoldSlotDef {
                    name,
                    locale_key: key,
                    icon: "",
                }
            })
            .collect();
        if !metadata.limbs.is_empty()
            && (metadata.limbs != limb_defs || metadata.hold_slots != slot_defs)
        {
            return Err(
                "Player prefab candidates have conflicting limb or inventory slot order".into(),
            );
        }
        metadata.limbs = limb_defs;
        metadata.hold_slots = slot_defs;
        if let (Some(index), Some(multiplier)) = (character, height_multiplier)
            && let Some(def) = metadata.characters.get_mut(index)
        {
            def.height_multiplier = Some(multiplier as f32);
        }
        // Saved body defaults differ slightly between character prefabs; use
        // the first character's so the template does not depend on asset order.
        if character == Some(0) || !body_defaults_from_prefab {
            metadata.body_defaults = defaults;
            body_defaults_from_prefab = true;
        }
    }
    if metadata.limbs.is_empty() {
        return Err("Cannot decode player prefab limb and inventory slot order".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_item_catalogue_is_backed_by_resource_prefabs() {
        let Some(game_dir) = std::env::var_os("CU_GAME_DIR") else {
            eprintln!("skipped: CU_GAME_DIR is not set");
            return;
        };
        let data_dir = Path::new(&game_dir).join("CasualtiesUnknown_Data");
        let assembly = CilObject::from_path_with_validation(
            data_dir.join("Managed").join("Assembly-CSharp.dll"),
            dotscope::ValidationConfig::minimal(),
        )
        .unwrap();
        let studio = Studio::open(&data_dir).unwrap();
        let resource_ids: std::collections::BTreeSet<_> =
            resource_item_ids(&studio).unwrap().into_values().collect();
        let catalogue_ids: std::collections::BTreeSet<_> = super::super::il::items(&assembly)
            .unwrap()
            .into_keys()
            .collect();
        assert!(catalogue_ids.is_subset(&resource_ids));
        // This leftover prefab has no GlobalItems entry; Item.GetItem cannot load it.
        assert_eq!(
            resource_ids
                .difference(&catalogue_ids)
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["reinforcedrope"]
        );
    }

    #[test]
    fn string_array_reads_alignment_and_rejects_truncation() {
        let mut bytes = 2_i32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&6_i32.to_le_bytes());
        bytes.extend_from_slice(b"bullet");
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&7_i32.to_le_bytes());
        bytes.extend_from_slice(b"battery");
        bytes.push(0);
        assert_eq!(
            Cursor {
                bytes: &bytes,
                offset: 0
            }
            .strings(),
            Some(vec!["bullet".into(), "battery".into()])
        );
        assert_eq!(
            Cursor {
                bytes: &bytes[..bytes.len() - 3],
                offset: 0
            }
            .strings(),
            None
        );
    }

    #[test]
    fn mono_header_rejects_truncated_alignment() {
        let mut bytes = vec![0; 28];
        bytes.extend_from_slice(&3_i32.to_le_bytes());
        bytes.extend_from_slice(b"Bag");
        bytes.push(0);
        assert_eq!(mono_behaviour_data_offset(&bytes), Some(36));
        assert_eq!(mono_behaviour_data_offset(&bytes[..35]), None);
        assert_eq!(mono_behaviour_data_offset(&bytes[..34]), None);
    }
}
