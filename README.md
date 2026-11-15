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
- **Game-synced item stats.** Select the installation folder from *Game data → Set game folder…* (or use automatic detection) to read item definitions from `Assembly-CSharp.dll` and container limits and encumbrance multipliers from Unity assets. Extraction reads metadata and IL without executing game code; the Unity asset layout currently supports the game's 2022 serialization format. A sync error is shown if the game data cannot be read.
- **Extensible interface languages.** English and Russian are built in; additional JSON language packs can be loaded without rebuilding. Choose *Game data → Interface language* independently of the language used for in-game names.

## Building

Requires a recent stable Rust toolchain.

```sh
cargo build --release
```

The executable is `target/release/cu_save_editor.exe`. API documentation is available with `cargo doc --open`.

## Usage

1. In the game, use **Save & Quit**. The game deletes `save.sv` as soon as a run is loaded, so only edit the save while the game is not running a loaded run.
2. Start the editor. It opens the default save automatically:
   `%USERPROFILE%\AppData\LocalLow\Orsoniks\CasualtiesUnknown\save.sv`.
   You can also pass a path on the command line or use *File → Open…* (Ctrl+O).
3. Edit, then press **Save** (Ctrl+S).
4. Load the run in the game.

Editor settings (game folder, game-name language, and interface language) are stored in `%APPDATA%\cu_save_editor\config.json`.

Item weights and supported wearable slots come from the installed game rather than the editor's fallback catalogue. Carrying capacity is computed from the saved body state and run settings; when stats are missing, the UI marks the estimate incomplete instead of claiming the character is under capacity.

Inventory shows two different totals. **Game load** is shown in the game's
weight units (`u`): item weights are combined with container encumbrance
multipliers, and liquid items scale with their fill level. Container capacity
and maximum item weight are storage limits; they do not clamp the carried
weight of contents already present in a save. These limits and the encumbrance
reduction are shown in each container's details. **Physical weight** is a
separate rough approximation: liquids are treated as 1 kg per litre (regardless
of type), and game weight units are treated as kilograms for empty item weight.
This estimate does not apply storage multipliers and is not the game's
encumbrance value. Missing or invalid liquid component data is marked
incomplete rather than assumed empty; the game may use default contents when
component data is absent.

## Interface language packs

Put UTF-8 `.json` files in `%APPDATA%\cu_save_editor\languages` on Windows
(on other platforms, in the `cu_save_editor/languages` folder under the OS
configuration directory). Create the folder if necessary. See
[`examples/interface-language.json`](examples/interface-language.json) for the format.
Each pack needs a unique `code`, a display `name`, and a `translations` object
mapping exact English interface strings to translated strings. Codes may contain
ASCII letters, digits and hyphens; `en` and `ru` are reserved. Keep placeholders
such as `{level}` unchanged. Missing or empty translations fall back to English.
The built-in [translation table](src/ui/interface_language.rs) lists many
available source strings; game-provided item and setting names are separate.

Restart the editor or use *Game data → Reload interface languages*, then select
the language in the same menu. Invalid files and duplicate codes are reported
in the status bar; valid packs remain available. If a selected pack is missing,
English is used until the pack is restored.

## Project layout

- `src/save.rs`: reading/writing (gzip or plain JSON), validation, backups.
- `src/json.rs`: JSON path and number helpers.
- `src/schema.rs`, `src/catalog.rs`: field metadata and the game's fallback item / liquid / setting catalogue.
- `src/game_data.rs`: game installation sync using `dotscope` metadata/IL parsing and `unity-rs-core` asset reading.
- `src/model.rs`: game-accurate operations (healing, skill levels, inventory grouping).
- `src/locale.rs`: game installation discovery and game language files.
- `src/interface_packs.rs`, `src/ui/interface_language.rs`: external and built-in interface translations.
- `src/ui/`: the egui views. `src/app.rs` and `src/app/view.rs` form the application shell.

## Testing

```sh
cargo test
```

`tests/real_save.rs` round-trips the example save in `Exemple/save.sv`, if it is present.
To also verify liquid item metadata against an installed game, set
`CU_GAME_DIR` to the game installation directory before running `cargo test`.
