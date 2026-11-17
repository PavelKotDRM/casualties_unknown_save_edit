use super::*;
use serde_json::{Value, json};
use std::{fs, path::PathBuf, time::SystemTime};

fn sample() -> Value {
    json!({
        "body": {"bloodVolume": 100.0, "corpsesSeen": 12, "skills": {"STR": 13}},
        "limbs": [{"skinHealth": 88.66524}],
        "items": [{"id": "waterbottle", "wearSlot": null, "label": "Фляга"}],
        "runTime": 7348.6626
    })
}

#[test]
fn new_game_generation_does_not_fabricate_game_defaults() {
    assert!(!can_create_new_game());
    assert!(try_new_game_data().is_err());
}

#[test]
fn gzip_round_trip_preserves_document_and_order() {
    let value = sample();
    let bytes = encode(&value, SaveFormat::Gzip);
    assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
    let (decoded, format) = decode(&bytes).unwrap();
    assert_eq!(format, SaveFormat::Gzip);
    assert_eq!(decoded, value);
    let keys: Vec<_> = decoded.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["body", "limbs", "items", "runTime"]);
    let text = crate::json::to_ascii_json(&decoded);
    assert!(text.contains("\"skinHealth\":88.66524"));
    assert!(text.contains("\"bloodVolume\":100.0"));
}

#[test]
fn plain_json_with_bom_is_accepted() {
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend(serde_json::to_vec(&sample()).unwrap());
    assert_eq!(decode(&bytes).unwrap().1, SaveFormat::PlainJson);
}

#[test]
fn non_save_json_is_rejected() {
    assert!(matches!(decode(b"{\"a\":1}"), Err(SaveError::NotASave(_))));
    assert!(matches!(decode(b"not json"), Err(SaveError::Json(_))));
}

#[test]
fn write_backs_up_externally_modified_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(SAVE_FILE_NAME);
    fs::write(&path, encode(&sample(), SaveFormat::Gzip)).unwrap();
    let first = create_backup(&path).unwrap();
    assert!(first.starts_with(backup_dir(&path)));
    let mut save = SaveFile::load(&path).unwrap();
    save.data["runTime"] = json!(1.0);
    assert_eq!(save.write().unwrap(), None);
    fs::write(&path, encode(&sample(), SaveFormat::Gzip)).unwrap();
    assert!(save.write().unwrap().unwrap().exists());
    assert_eq!(SaveFile::load(&path).unwrap().data["runTime"], json!(1.0));
    assert_eq!(list_backups(&path).len(), 2);
}

#[test]
fn backups_know_their_save_and_only_backups_can_be_deleted() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("my_run.sv");
    // Backup management is independent of the save format.
    fs::write(&path, b"backup source").unwrap();
    let first = create_backup(&path).unwrap();
    let second = create_backup(&path).unwrap();
    assert_ne!(first, second);
    assert_eq!(backup_source(&first), Some(path.clone()));
    assert_eq!(backup_source(&second), Some(path.clone()));
    assert_eq!(backup_source(&path), None);
    assert!(delete_backup(&path).is_err());
    assert!(path.exists());
    delete_backup(&first).unwrap();
    assert_eq!(list_backups(&path).len(), 1);
}

#[test]
fn summary_reads_the_run_overview() {
    let summary = SaveSummary::of(&sample());
    assert_eq!(summary.items, 1);
    assert_eq!(summary.run_time, Some(7348.6626));
    assert_eq!(summary.blood, Some(100.0));
    assert_eq!(summary.skills.len(), crate::schema::SKILLS.len());
    assert_eq!(summary.biome, None);
}

#[test]
fn cleanup_rules_pick_the_old_backups() {
    let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100 * 86_400);
    let day = |days: u64| Some(now - std::time::Duration::from_secs(days * 86_400));
    let list: Vec<_> = [day(0), day(2), day(10), None]
        .into_iter()
        .enumerate()
        .map(|(index, modified)| BackupInfo {
            path: PathBuf::from(format!("b{index}")),
            modified,
            size: 0,
        })
        .collect();
    let names = |paths: Vec<PathBuf>| {
        paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(old_backups(&list, Cleanup::KeepNewest(2), now)),
        ["b2", "b3"]
    );
    assert!(old_backups(&list, Cleanup::KeepNewest(9), now).is_empty());
    assert_eq!(
        names(old_backups(&list, Cleanup::OlderThanDays(1), now)),
        ["b1", "b2"]
    );
    assert_eq!(
        names(old_backups(&list, Cleanup::OlderThanDays(0), now)),
        ["b1", "b2"]
    );
}
