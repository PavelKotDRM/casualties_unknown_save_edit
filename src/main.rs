//! Entry point of the Casualties Unknown save editor.
//!
//! Usage: `cu_save_editor [path-to-save]`. Without an argument the game's
//! save (`save.sv` in [`cu_save_editor::platform::save_dir`]) is opened if it
//! exists.
//! `--export-interface-language <path>` writes an editable interface language pack.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use cu_save_editor::app::EditorApp;

const HELP: &str = "Usage: cu_save_editor [path-to-save]
       cu_save_editor --export-interface-language <output.json>
       cu_save_editor --help

Export an interface language template without opening the editor or loading the game.
Existing output files are not overwritten.";

fn cli(arguments: &[std::ffi::OsString]) -> Option<Result<(), String>> {
    let first = arguments.first()?.to_str()?;
    match first {
        "--help" | "-h" => Some(if arguments.len() == 1 {
            println!("{HELP}");
            Ok(())
        } else {
            Err("The help command takes no additional arguments.".into())
        }),
        "--export-interface-language" => Some(if let [_, path] = arguments {
            cu_save_editor::interface_packs::export_template(std::path::Path::new(path)).map(
                |count| {
                    println!(
                        "Exported {count} interface strings to {}.",
                        std::path::Path::new(path).display()
                    )
                },
            )
        } else {
            Err("Usage: cu_save_editor --export-interface-language <output.json>".into())
        }),
        flag if flag.starts_with('-') => Some(Err(format!("Unknown option: {flag}\n{HELP}"))),
        _ => None,
    }
}

fn attach_cli_console() {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn AttachConsole(process_id: u32) -> i32;
        }
        // Release builds are GUI executables; reuse the invoking terminal if available.
        unsafe {
            AttachConsole(u32::MAX);
        }
    }
}

fn main() -> eframe::Result {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments
        .first()
        .and_then(|argument| argument.to_str())
        .is_some_and(|argument| argument.starts_with('-'))
    {
        attach_cli_console();
    }
    if let Some(result) = cli(&arguments) {
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    cu_save_editor::diagnostics::install_panic_hook();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CU Save Editor")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 560.0]),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Casualties Unknown Save Editor",
        options,
        Box::new(|cc| Ok(Box::new(EditorApp::new(cc)))),
    )
}
