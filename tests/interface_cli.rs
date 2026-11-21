use std::process::Command;

#[path = "../build.rs"]
mod interface_strings;

fn editor() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cu_save_editor"))
}

#[test]
fn command_exports_a_pack_without_game_data_and_does_not_overwrite_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("translation template.json");
    let output = editor()
        .current_dir(directory.path())
        .args(["--export-interface-language"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let (packs, errors) = cu_save_editor::interface_packs::load(directory.path());
    assert!(errors.is_empty());
    assert_eq!(packs.len(), 1);
    assert_eq!(packs[0].translations["Blood oxygen"], "Blood oxygen");
    let original = std::fs::read(&path).unwrap();
    let repeated = editor()
        .arg("--export-interface-language")
        .arg(&path)
        .output()
        .unwrap();
    assert!(!repeated.status.success());
    assert!(
        String::from_utf8(repeated.stderr)
            .unwrap()
            .contains("Cannot create")
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn invalid_arguments_fail_and_help_does_not_start_the_gui() {
    for arguments in [
        vec!["--export-interface-language"],
        vec!["--export-interface-language", "one.json", "two.json"],
        vec!["--unknown-option"],
    ] {
        let output = editor().args(arguments).output().unwrap();
        assert!(!output.status.success());
        assert!(!output.stderr.is_empty());
    }
    let help = editor().arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("--export-interface-language")
    );
}
