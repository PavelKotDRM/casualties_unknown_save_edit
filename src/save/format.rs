use std::io::{Read, Write};

use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde_json::Value;

use super::SaveError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFormat {
    Gzip,
    PlainJson,
}

/// Decodes JSON, recognizing gzip and an optional UTF-8 BOM.
pub fn decode(bytes: &[u8]) -> Result<(Value, SaveFormat), SaveError> {
    let (text, format) = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut text = Vec::new();
        GzDecoder::new(bytes)
            .read_to_end(&mut text)
            .map_err(SaveError::Decompress)?;
        (text, SaveFormat::Gzip)
    } else {
        (bytes.to_vec(), SaveFormat::PlainJson)
    };
    let text = text.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&text);
    let value = serde_json::from_slice(text).map_err(SaveError::Json)?;
    validate(&value)?;
    Ok((value, format))
}

/// A format check only; unfamiliar fields and game versions are preserved.
pub fn validate(value: &Value) -> Result<(), SaveError> {
    let object = value
        .as_object()
        .ok_or_else(|| SaveError::NotASave("the root is not a JSON object".into()))?;
    for key in ["body", "limbs", "items"] {
        if !object.contains_key(key) {
            return Err(SaveError::NotASave(format!("missing \"{key}\" section")));
        }
    }
    Ok(())
}

pub fn encode(value: &Value, format: SaveFormat) -> Vec<u8> {
    let text = crate::json::to_ascii_json(value);
    match format {
        SaveFormat::PlainJson => text.into_bytes(),
        SaveFormat::Gzip => {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
            encoder
                .write_all(text.as_bytes())
                .expect("writing to a Vec cannot fail");
            encoder.finish().expect("writing to a Vec cannot fail")
        }
    }
}

pub fn to_pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).expect("serialising a Value cannot fail")
}
