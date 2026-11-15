//! Helpers for working with the dynamically-typed save JSON.
//!
//! The editor keeps the whole save as a [`serde_json::Value`] (with insertion
//! order preserved) instead of a strongly typed struct. This guarantees that
//! fields unknown to the editor — e.g. ones added by future game updates — are
//! never lost when the file is written back.
//!
//! The game itself deserialises the file with Newtonsoft.Json into C# `float`
//! and `int` fields, so this module takes care to keep integers as integers
//! and to round floats to `f32` precision when values are edited.

use serde_json::{Map, Number, Value};

/// Converts a value to a JSON number that is stored with `f32` precision.
///
/// The shortest decimal representation of the `f32` is used, so editing a
/// field to `33.3` writes `33.3` instead of `33.299999237060547`.
/// Non-finite values are mapped to `0.0`, because JSON cannot represent them.
pub fn f32_number(value: f64) -> Number {
    let f = value as f32;
    if !f.is_finite() {
        return Number::from_f64(0.0).expect("0.0 is finite");
    }
    let shortest: f64 = f.to_string().parse().unwrap_or(f as f64);
    Number::from_f64(shortest).expect("finite f32 converts to finite f64")
}

/// Returns `true` if the number is stored as an integer in the JSON text.
pub fn is_integer(number: &Number) -> bool {
    number.is_i64() || number.is_u64()
}

/// Writes `new_value` into `slot`, preserving the integer/float kind that the
/// slot already had.
///
/// * integer slots receive the rounded value,
/// * float slots receive an `f32`-rounded value (see [`f32_number`]),
/// * non-numeric slots become floats.
pub fn set_number_preserving(slot: &mut Value, new_value: f64) {
    let keep_int = matches!(slot, Value::Number(n) if is_integer(n));
    *slot = if keep_int {
        Value::Number(Number::from(new_value.round() as i64))
    } else {
        Value::Number(f32_number(new_value))
    };
}

/// Reads a field of an object as `f64` (integers are converted).
pub fn get_f64(obj: &Map<String, Value>, key: &str) -> Option<f64> {
    obj.get(key).and_then(Value::as_f64)
}

/// Reads a field of an object as `i64`. Floats are truncated.
pub fn get_i64(obj: &Map<String, Value>, key: &str) -> Option<i64> {
    obj.get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
}

/// Reads a boolean field of an object.
pub fn get_bool(obj: &Map<String, Value>, key: &str) -> Option<bool> {
    obj.get(key).and_then(Value::as_bool)
}

/// Reads a string field of an object (`null` reads as `None`).
pub fn get_str<'a>(obj: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(Value::as_str)
}

/// Sets a numeric field, preserving the existing integer/float kind.
///
/// If the key does not exist yet it is created as a float.
pub fn set_f64(obj: &mut Map<String, Value>, key: &str, value: f64) {
    let slot = obj.entry(key.to_owned()).or_insert(Value::Null);
    set_number_preserving(slot, value);
}

/// Sets an integer field.
pub fn set_i64(obj: &mut Map<String, Value>, key: &str, value: i64) {
    obj.insert(key.to_owned(), Value::Number(Number::from(value)));
}

/// Sets a boolean field.
pub fn set_bool(obj: &mut Map<String, Value>, key: &str, value: bool) {
    obj.insert(key.to_owned(), Value::Bool(value));
}

/// Short human readable type name of a JSON value, used by the raw viewer.
pub fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(n) if is_integer(n) => "int",
        Value::Number(_) => "float",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Serialises a value to compact JSON in which every non-ASCII character is
/// escaped as `\uXXXX`.
///
/// The game reads and writes the save with `Encoding.ASCII`, so raw UTF-8
/// bytes would be corrupted into `?` characters. Escaped sequences are
/// understood by Newtonsoft.Json and survive the ASCII round-trip.
pub fn to_ascii_json(value: &Value) -> String {
    let compact = serde_json::to_string(value).expect("serialising a Value cannot fail");
    escape_non_ascii(&compact)
}

/// Escapes every non-ASCII character of an (already valid) JSON text.
///
/// Non-ASCII characters can only occur inside JSON strings, so replacing
/// them with `\uXXXX` escapes (UTF-16 surrogate pairs where needed) keeps the
/// document valid and semantically identical.
pub fn escape_non_ascii(json: &str) -> String {
    if json.is_ascii() {
        return json.to_owned();
    }
    let mut out = String::with_capacity(json.len() + 16);
    let mut buf = [0u16; 2];
    for ch in json.chars() {
        if ch.is_ascii() {
            out.push(ch);
        } else {
            for unit in ch.encode_utf16(&mut buf) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    out
}

/// One step of a path into a JSON document.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PathSeg {
    /// Object member.
    Key(String),
    /// Array element.
    Index(usize),
}

impl std::fmt::Display for PathSeg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathSeg::Key(k) => write!(f, ".{k}"),
            PathSeg::Index(i) => write!(f, "[{i}]"),
        }
    }
}

/// Formats a path as `$.body.skills.STR` / `$.items[3].id`.
pub fn path_to_string(path: &[PathSeg]) -> String {
    let mut s = String::from("$");
    for seg in path {
        s.push_str(&seg.to_string());
    }
    s
}

/// Resolves a mutable reference to the value at `path`.
pub fn get_path_mut<'a>(root: &'a mut Value, path: &[PathSeg]) -> Option<&'a mut Value> {
    let mut cur = root;
    for seg in path {
        cur = match seg {
            PathSeg::Key(k) => cur.as_object_mut()?.get_mut(k)?,
            PathSeg::Index(i) => cur.as_array_mut()?.get_mut(*i)?,
        };
    }
    Some(cur)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn float_edits_use_shortest_f32_representation() {
        let mut v = json!(1.0);
        set_number_preserving(&mut v, 33.3);
        assert_eq!(v.to_string(), "33.3");
        set_number_preserving(&mut v, 100.0);
        assert_eq!(
            v.to_string(),
            "100.0",
            "whole floats keep the decimal point"
        );
    }

    #[test]
    fn integer_fields_stay_integers() {
        let mut v = json!(12);
        set_number_preserving(&mut v, 13.6);
        assert_eq!(v.to_string(), "14");
    }

    #[test]
    fn non_ascii_is_escaped_for_the_games_ascii_reader() {
        let v = json!({"label": "Ящик 📦"});
        let s = to_ascii_json(&v);
        assert!(s.is_ascii());
        assert_eq!(serde_json::from_str::<Value>(&s).unwrap(), v);
    }

    #[test]
    fn path_resolution() {
        let mut v = json!({"items": [{"id": "a"}, {"id": "b"}]});
        let path = [
            PathSeg::Key("items".into()),
            PathSeg::Index(1),
            PathSeg::Key("id".into()),
        ];
        assert_eq!(path_to_string(&path), "$.items[1].id");
        *get_path_mut(&mut v, &path).unwrap() = json!("c");
        assert_eq!(v["items"][1]["id"], "c");
    }
}
