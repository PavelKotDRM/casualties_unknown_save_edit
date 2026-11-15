# Casualties Unknown Save Editor

A desktop save editor for **Casualties Unknown**, written in Rust with [egui](https://github.com/emilk/egui) 0.37.

The game stores its run in `save.sv`, which is gzip-compressed JSON. The editor opens it, makes a backup, and shows the data as graphical, game-like views: gauges, a clickable body figure, skill bars, inventory cards and so on. A raw JSON tree is also available. Any field the editor doesn't recognise is kept unchanged, and an unmodified file is written back byte-for-byte identical.

## Features

| Tab | What you can do |
| --- | --- |
| **Overview** | Body figure, vitals, character and run summary, and one-click quick fixes. |
| **Character** | Character type, height, age and id. Skills (STR / RES / INT) with level ± buttons that recalculate XP bounds exactly as the game does. |
| **Health** | All body values grouped into cards (vitals, needs, blood, temperature, …) with coloured gauges you can drag. Restore vitals and fill needs. |
| **Limbs** | Painted body figure coloured by condition, with markers for bleeding, infection, fracture, dislocation, tourniquet, dismemberment and prosthetics. Click a limb to edit it, or heal one or all limbs, including regrowing dismembered ones. |
| **Inventory** | Items grouped by slot or worn location, with contents, verified weight, armor and insulation stats. Compatible wear slots are offered when moving or adding equipment; existing incompatible assignments are retained and flagged for repair. The editor calculates carrying load and warns when the character is overburdened. |
| **Run** | Layer, time, distance and calories. All run settings with sliders, checkboxes and dropdowns. Mood chart with draggable points. Saved recipes. |
| **Raw JSON** | Collapsible / expandable tree with filtering, type changes, a context menu (copy path, copy JSON, delete) and a full text editor with validation. |

Other features:

- **Backups.** A timestamped copy is made in a `SaveEditorBackups` folder next to the save every time a file is opened. You can also make backups manually, and browse or restore them from *File → Backups…*.
- **Undo / redo** (Ctrl+Z / Ctrl+Y). Unsaved changes are protected when you open another file or close the window.
- **Import / export** a pretty-printed, human-readable JSON copy (*File → Export JSON…* / *Import JSON…*).
- **Real item, liquid and setting names** in any game language. These are read from the game's `Lang/*.json` files when the game folder is found. Choose the language and the folder in the *Game data* menu.
- **Game-synced item stats.** Select the installation folder from *Game data → Set game folder…* (or use automatic detection) to read item definitions from `Assembly-CSharp.dll` and container multipliers from Unity assets. Extraction reads metadata and IL without executing game code; the Unity asset layout currently supports the game's 2022 serialization format. A sync error is shown if the game data cannot be read.
- **English and Russian interface.** Choose *Game data → Interface language*; this preference is saved separately from the language used for in-game names.

## Building

Requires a recent stable Rust toolchain.

```sh
cargo build --release
```

The executable is `target/release/cu_save_editor.exe`. API documentation is available with `cargo doc --open`.

## Usage

1. In the game, use **Save & Quit**. The game deletes `save.sv` as soon as a run is loaded, so only edit the save while the game is not running a loaded run.
2. Start the editor. It opens the default save automatically:
   `C:\Users\<user>\AppData\LocalLow\Orsoniks\CasualtiesUnknown\save.sv`.
   You can also pass a path on the command line or use *File → Open…* (Ctrl+O).
3. Edit, then press **Save** (Ctrl+S).
4. Load the run in the game.

Editor settings (game folder, game-name language, and interface language) are stored in `%APPDATA%\cu_save_editor\config.json`.

Item weights and supported wearable slots come from the installed game rather than the editor's fallback catalogue. Carrying capacity is computed from the saved body state and run settings; when stats are missing, the UI marks the estimate incomplete instead of claiming the character is under capacity.

## Project layout

- `src/save.rs`: reading/writing (gzip or plain JSON), validation, backups.
- `src/json.rs`: JSON path and number helpers.
- `src/schema.rs`, `src/catalog.rs`: field metadata and the game's fallback item / liquid / setting catalogue.
- `src/game_data.rs`: game installation sync using `dotscope` metadata/IL parsing and `unity-rs-core` asset reading.
- `src/model.rs`: game-accurate operations (healing, skill levels, inventory grouping).
- `src/locale.rs`: game installation discovery and language files.
- `src/ui/`: the egui views. `src/app.rs` is the application shell.

## Testing

```sh
cargo test
```

`tests/real_save.rs` round-trips the example save in `Exemple/save.sv`, if it is present.
