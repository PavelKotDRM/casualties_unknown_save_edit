//! Round-trip check against a real save, if one is available locally
//! (`Exemple/save.sv` is git-ignored, so the test is skipped otherwise).

use std::io::Read;
use std::path::Path;

use cu_save_editor::save::{self, SaveFormat};

#[test]
fn real_save_round_trips_to_identical_json() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Exemple")
        .join("save.sv");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipped: {} not found", path.display());
        return;
    };
    let mut original = String::new();
    flate2::read::GzDecoder::new(&bytes[..])
        .read_to_string(&mut original)
        .unwrap();

    let (data, format) = save::decode(&bytes).unwrap();
    assert_eq!(format, SaveFormat::Gzip);
    let encoded = save::encode(&data, format);
    let mut text = String::new();
    flate2::read::GzDecoder::new(&encoded[..])
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(
        text, original,
        "re-encoded JSON differs from the game's output"
    );
}
