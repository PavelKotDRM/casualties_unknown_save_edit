# Casualties Unknown Save Editor

[![CI](https://github.com/PavelKotDRM/casualties_unknown_save_edit/actions/workflows/ci.yml/badge.svg)](https://github.com/PavelKotDRM/casualties_unknown_save_edit/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Steam](https://img.shields.io/badge/Steam-Casualties%3A%20Unknown-1b2838?logo=steam)](https://store.steampowered.com/app/4576490/)

A desktop save editor and wiki for
[**Casualties: Unknown**](https://store.steampowered.com/app/4576490/) by
Orsoniks, written in Rust with [egui](https://github.com/emilk/egui).

The game stores its run in `save.sv`, which is gzip-compressed JSON. The editor
opens it, makes a backup, and shows the data as game-like views: gauges, a
clickable body figure, skill bars, inventory cards and more. A raw JSON tree is
also available. Fields the editor does not recognise are kept unchanged, and an
unmodified file is written back byte-for-byte identical.
Opening or reloading a save does not count as an edit; the save prompt appears
only when the document has unsaved changes.

## Contents

- [Casualties Unknown Save Editor](#casualties-unknown-save-editor)
  - [Contents](#contents)
  - [Features](#features)
    - [Editor tabs](#editor-tabs)
    - [Game progress](#game-progress)
    - [Tools](#tools)
    - [Game data](#game-data)
  - [Getting started](#getting-started)
    - [Download](#download)
    - [Build from source](#build-from-source)
    - [Editing a save](#editing-a-save)
  - [Inventory and weight](#inventory-and-weight)
  - [Platforms](#platforms)
  - [Reporting problems](#reporting-problems)
    - [Privacy](#privacy)
  - [Interface language packs](#interface-language-packs)
  - [Development](#development)
    - [Project layout](#project-layout)
    - [Updating for a new game version](#updating-for-a-new-game-version)
    - [Testing](#testing)
    - [CI/CD and releases](#cicd-and-releases)
  - [License](#license)

## Features

### Editor tabs

| Tab | What you can do |
| --- | --- |
| **Overview** | Body figure, vitals, character and run summary, one-click quick fixes. |
| **Character** | Character, skills with exact XP bounds, height, age and id; unlocked characters and endings. |
| **Health** | All body values in cards (vitals, needs, blood, temperature, …) with draggable gauges. |
| **Limbs** | Body figure coloured by condition; heal limbs, regrow dismembered ones, edit injuries. |
| **Inventory** | Items by hold slot or worn location, contents, weight, armor and insulation. |
| **Run** | Layer, time, distance, calories, all run settings, mood chart and saved recipes. |
| **Raw JSON** | Filterable tree with type changes, a context menu and a validating text editor. |

### Game progress

- **Character unlocks.** The game keeps them outside the save, in its own
  settings (Unity PlayerPrefs): winning a run as character *N* sets
  `wonchar<N>`, which unlocks character *N + 1*. Descending raises
  `deepestlayer` to the deepest layer reached. The editor reads and changes
  these values on the *Character* tab. Close the game first, because it caches
  its settings while running.
- **Endings.** The game records each ending it has seen in the same settings.
  The *Endings* card, the wiki and *Help → About* show discovered progress
  without revealing the total. In the card or wiki, explicitly request a hint
  for another ending, then reveal further details one click at a time.
  Available stages are the local Steam achievement description, the game's
  ending name, its ending-screen status, and how-to text when supplied by the
  game data. Missing stages are skipped; the editor does not invent walkthroughs.
  Revealing hints does not mark endings as discovered or change game settings.
- **Achievements** (*Help → Achievements*). The game's 40 Steam achievements
  with overall progress, unlock dates, search and an All / Unlocked / Locked
  filter. They are read, read-only, from Steam's local stats cache. Hidden
  achievements stay hidden until unlocked.
- **No spoilers by default.** Unreached layers and undiscovered endings are
  omitted, including from wiki search and the new-save layer picker, so counting
  placeholders cannot spoil their totals. Separate **Reveal total layers** and
  **Reveal total endings** buttons opt into counts and locked placeholders, not
  hidden names or hints. Characters still use locked placeholders.
  Reveal choices persist across restarts. Use **Hide ending hints** or the
  corresponding **Hide … total** button to conceal them again.
- **Unavailable progress stays unknown.** Missing game data or unreadable
  preferences never count as completed progress. Unknown entries do not reveal
  character, layer or ending spoilers.

### Tools

- **New save wizard** (*File → New save…*). Pick a game-defined world preset or
  customise every setting, choose the starting layer (limited to layers already
  reached in the game) and configure the character (limited to unlocked
  characters).
- **Game wiki** (*Help → Game wiki*, F1). A searchable reference built from the
  installed game's metadata and language files: characters, endings, skills
  and the XP table, body and limb stats with healthy ranges, status effects,
  world settings, presets, layers, items and liquids. Works without an open
  save. Item and medicine entries list value, spoilage, crafting qualities,
  contents and the effects of use (per use or per whole item, timed effects,
  possible random or overdose effects), read from the game's code. Search
  matches every word, ranks title matches first and highlights hits (Ctrl+F,
  Esc to clear). Articles and search results are cached; only visible cards
  are laid out, including in large item listings.
  Status cards use localized names and the installed game's background palette
  (including its neutral and special colours), not editor-authored medical
  classifications. Dynamic or unresolved severity uses the native neutral shade.
  Item/liquid stat names prefer the game's language file; matching interface
  strings and missing labels use the selected interface language, including
  built-in Russian stat translations.
  Unknown fields fall back to readable names. Effect colours use game-derived
  healthy bounds and status guards. Ambiguous changes, such as raising an already
  unknown body temperature, remain neutral.
- **Backups** (*File → Backups…*). A timestamped copy goes to a
  `SaveEditorBackups` folder next to the save whenever a file is opened,
  restored over or overwritten after an outside change. The browser compares any
  backup with the current save, restores, opens or deletes it, and cleans up
  several at once (keep the newest *N*, or older than *N* days).
- **Automatic refresh.** Every 5 seconds the editor re-reads what the game can
  change: unlocks, endings, achievements, backups and the game installation. An
  unmodified save that changes on disk is reloaded; an edited one is left alone
  with a warning. Toggle it in *Game data → Refresh data automatically*;
  *Refresh now* (F5) always works.
- **Undo / redo** (Ctrl+Z / Ctrl+Y), with protection for unsaved changes.
- **Import / export** of a pretty-printed JSON copy (*File → Export JSON…* /
  *Import JSON…*).
- **Version information.** The status bar shows the editor and game versions.
  *Help → About* lists the editor build, the game version it was made for, and
  the installed game's version, Unity version, Steam build, folder and size. A ⚠
  warning appears when the installed game is a different version.
- **Problem reports** (*Help → Report a problem…*). See [Reporting
  problems](#reporting-problems).

### Game data

- **Real names** of items, liquids and settings in any game language, read from
  the game's `Lang/*.json` files.
- **Readable health indicators.** Vital signs, health and limb fields use
  localized names rather than internal IDs, with readable interface-language
  fallbacks when the game has no label. Original JSON keys remain in tooltips
  and the raw editor.
- **Game-synced item stats.** Item definitions are read from
  `Assembly-CSharp.dll`, and container limits and encumbrance multipliers from
  the Unity assets. The editor reads metadata and IL only and never runs game
  code. Set the folder with *Game data → Set game folder…* or let the editor
  find it.
  All subclasses of the game's item definition type are included, including
  loose small, medium and large batteries; installed batteries are separate
  device component state.
- **An installation is required for game-dependent views.** Without readable
  game data, the editor shows an installation prompt instead of a built-in
  game catalogue. Opening, backing up, importing, exporting and editing raw
  save JSON remain available.
- **Interface languages.** English and Russian are built in, and JSON packs add
  more without rebuilding (see [Interface language
  packs](#interface-language-packs)).

## Getting started

### Download

Ready-made builds for Windows, Linux and macOS are attached to each
[release](https://github.com/PavelKotDRM/casualties_unknown_save_edit/releases).
Every CI run also uploads build artifacts.

### Build from source

Requires a recent stable [Rust](https://rustup.rs) toolchain. On Linux, install
the development packages first (Debian/Ubuntu names: `pkg-config
libfontconfig1-dev libwayland-dev libxkbcommon-dev`).

```sh
cargo build --release
```

The executable is `target/release/cu_save_editor` (`cu_save_editor.exe` on
Windows). API documentation is available with `cargo doc --open`.

### Editing a save

1. In the game, use **Save & Quit**. The game deletes `save.sv` as soon as a run
   is loaded, so edit the save only while no run is loaded.
2. Start the editor. It opens the default save automatically (see
   [Platforms](#platforms)). You can also pass a path on the command line or use
   *File → Open…* (Ctrl+O).
3. Make your changes and press **Save** (Ctrl+S).
4. Load the run in the game.

To start from scratch, use *File → New save…* and save the result as `save.sv`
in the game's save folder. Defaults come from the installed game; generated
identity fields use the lower bounds of the game's random ranges and can be
edited before saving.

Editor settings (game folder, languages, automatic refresh, spoiler reveals) are stored in
`cu_save_editor/config.json` in the system configuration folder: `%APPDATA%` on
Windows, `~/.config` on Linux and `~/Library/Application Support` on macOS.

## Inventory and weight

Adding an item uses readable hand/body-slot labels and includes game-defined
liquid contents (including medicines and drinks), installed batteries, and
ammunition component defaults from resource prefabs. Empty-by-default containers
stay empty; weapons are not automatically loaded beyond their game defaults.
Contents remain editable in the item details. Liquid fill levels are included
when checking whether a new item fits inside a container.

The Limbs tab calculates protection for every body part from the game's worn
item coverage (including secondary limbs), character and run bonuses, and
prosthetics. Armor is shown as the game's damage divisor and the resulting
percentage of damage for effects that actually use armor; it is not a universal
resistance. Clothing insulation is a whole-body contribution in the game,
not per-limb protection. Total insulation is shown both with and without a
sleeping bag because the game does not save that temporary state. The
calculation reports missing or invalid data
instead of assuming equipment or inventing separate damage-type resistances.

Liquid edits update condition using the game's total-volume/capacity formula;
the condition control is read-only for these items. Fixed-cost consumables have
remaining-use controls linked to condition in both directions, using the game's
condition thresholds and single-precision subtraction. Minigames and actions
that transfer or destroy the item are not treated as repeatable uses; other
gameplay requirements still apply.

Installed-battery choices include every compatible battery from the game data.
Maximum charge follows the selected battery, and charge and condition are linked
by `charge = condition × maximum charge`. Changing battery type preserves its
condition percentage; removing it clears charge and its favourite flag. These
are save edits, not transfers of loose batteries from inventory.
Existing inconsistencies can be repaired with **Synchronize with contents**;
merely opening item details does not rewrite them. Ammunition counts remain
independent of condition, as in the game. The Raw JSON tab remains unrestricted.

The editor follows the game's own rules:

- Items are grouped by the six hold slots (right hand, left hand, mouth, upper,
  middle and lower back) or by worn location.
- Hands-only items (toolbox, water jug, pickaxes, sledgehammer and others
  flagged `onlyHoldInHands`) go only in the hands; clothing is worn in its own
  wear slot. Both may still be stored inside containers.
- Clothing cannot be worn on a dismembered limb, and a container that still
  holds items cannot go into another container.
- Containers accept items by the game's tags, per-item weight limits and
  remaining capacity; full or incompatible containers are disabled.
- Warnings flag items in non-existent slots and condition-0 items that the game
  destroys. Existing invalid assignments are kept and flagged for repair.

The inventory shows two totals:

- **Game load**, in the game's weight units (`u`). Item weights are combined
  with container encumbrance multipliers, and liquids scale with their fill
  level. This is what the game uses for overburdening.
- **Physical weight**, a rough estimate: liquids count as 1 kg per litre and
  game units as kilograms. It ignores storage multipliers.

Missing item stats or liquid data mark a total as incomplete rather than
guessing.

**Balance containers** redistributes stored items while leaving equipped holders
in place. Compatible contents of valid specialized containers (identified by
the game's tag restrictions, including ammunition holders) also stay put.
Choose one of three objectives for the remaining items: minimum carrying load,
equal stored weight, or equal fill percentage. Favourites stay pinned unless **Move
favourites** is selected. Capacity, per-item limits, tags and liquid weights are
respected; item components move with their items. Existing over-capacity storage
can be repaired when a valid distribution exists.

The balancer searches for an optimum for indivisible items, not a greedy
approximation. To keep the interface responsive, it stops after 200,000 search
nodes (and 256 recursive assignment levels). If a limit is reached or no valid
packing exists, it reports an error and leaves the save unchanged. Pin more
items or choose another objective to reduce the search. Balancing can be undone
normally.

**Reload from inventory** appears in an ammunition box, magazine or weapon's
details. It transfers compatible, non-favourite ammunition already in the
inventory; it never creates ammunition or magazines. Loose rounds are used
first, then rounds from boxes or spare magazines. Empty boxes and magazines
remain in the inventory. A weapon without a magazine uses a compatible spare
when available; reloading also chambers a round, ejects a spent casing and
closes the action without changing the safety. Capacities, ammunition types,
feed types and chamber encodings come from the installed game. Consumed items
and their components are removed together; reloads can be undone.

## Platforms

The editor is developed and tested on Windows. The code is portable, and
everything specific to an operating system is kept in `src/platform.rs`.

| | Windows | Linux / Steam Deck (Proton) |
| --- | --- | --- |
| Save | `%USERPROFILE%\AppData\LocalLow\Orsoniks\CasualtiesUnknown\save.sv` | the same path inside the Proton prefix, `steamapps/compatdata/4576490/pfx/drive_c/users/steamuser/…` |
| Game folder | from `Player.log` or the Steam libraries | from the Steam libraries (native, `~/.local/share/Steam`, Flatpak) |
| Unlocks and endings | read and changed | read and changed in the prefix (close the game first) |

On macOS, and for native Unity builds, the editor looks for the save in Unity's
standard data folder. Reading the game's settings is not supported there yet, so
progress is shown as unknown, not completed.

To add a platform, extend the functions in `src/platform.rs` and, for progress
tracking, add a `PrefsStore` implementation in `src/player_prefs.rs`.

## Reporting problems

Use *Help → Report a problem…*, or the 🐞 button next to an error in the status
bar. The window lists problems the editor has detected (for example, a missing
game folder or an unsupported game version) and collects debugging information:

- the editor, operating-system and game versions;
- which data sources were found and whether they could be read;
- recent error messages and the last crash report, if any;
- optionally, a progress summary: unlocked characters, endings, achievements,
  and the open save's character, layer and run time.

**Create GitHub issue…** opens a pre-filled [bug
report](https://github.com/PavelKotDRM/casualties_unknown_save_edit/issues/new?template=bug_report.yml)
in your browser, where you can review it before submitting. If the information
is too long for a link, it is copied to the clipboard so you can paste it into
the form. You can also copy the report or save it to a file.

### Privacy

- Nothing is sent automatically. The text shown in the window is exactly what
  will be shared, and you can edit it.
- Your user and computer names, home folder, other users' folders, Steam account
  ids and e-mail addresses are replaced with placeholders such as `~`, `<user>`
  and `<steam-account>`.
- Save contents and backups are never included. The progress summary is off
  unless you enable it.
- If the editor crashes, a redacted crash report is written to
  `cu_save_editor/crashes` in the system configuration folder. It stays on your
  computer until you include it in a report or delete it with *Delete crash
  reports*.

GitHub issues are public. Do not attach save files you would not want others to
see.

## Interface language packs

Put UTF-8 `.json` files in the `cu_save_editor/languages` folder of the system
configuration folder (create it if necessary). See
[`examples/interface-language.json`](examples/interface-language.json) for the
format:

- `code`: a unique code of ASCII letters, digits and hyphens (`en` and `ru` are
  reserved);
- `name`: the display name;
- `translations`: exact English interface strings mapped to translated strings.
  Keep placeholders such as `{level}` unchanged. Missing or empty translations
  fall back to English.

Export a complete template of the editor's interface translation keys:

```powershell
.\cu_save_editor.exe --export-interface-language .\interface-language.json
```

From source, use `cargo run -- --export-interface-language .\interface-language.json`.
The command does not open the GUI or require a game installation. It refuses to
overwrite an existing file. Change `code` and `name`, then translate the English
values under `translations`, leaving keys and placeholders unchanged.

The template is generated from the application source at build time, including
menus, tooltips, editor parameter labels and built-in fallback stat labels.
Game-provided item names, descriptions and settings are still translated by the
game's language files; they are not exported.
The exported [`examples/interface-language-template.json`](examples/interface-language-template.json)
contains the current English keys and values, including readable health labels.

Restart the editor or use *Game data → Reload interface languages*, then select
the language in the same menu. Invalid files and duplicate codes are reported in
the status bar.

## Development

### Project layout

| Path | Purpose |
| --- | --- |
| `src/save.rs`, `src/save/` | Reading and writing saves (gzip or plain JSON), validation, backups. |
| `src/json.rs` | JSON path and number helpers. |
| `src/schema.rs`, `src/schema/` | Field metadata, characters, layers, endings, world settings and presets. |
| `src/catalog.rs` | Runtime item and liquid catalogue from the selected installation. |
| `src/game_data.rs`, `src/game_data/` | Item stats from the game's assemblies and Unity assets; item and liquid effects from the game's code. |
| `src/model.rs`, `src/model/` | Healing, skill levels, weights, shared placement validation and container balancing. |
| `src/locale.rs` | Game installation discovery and language files. |
| `src/game_info.rs` | Installed game, Unity and Steam versions; supported game version. |
| `src/achievements.rs` | Steam achievements from Steam's binary stats cache. |
| `src/player_prefs.rs` | The game's settings outside the save, one backend per platform. |
| `src/platform.rs` | Everything that depends on the operating system. |
| `src/diagnostics.rs` | Redaction, error log, problem reports and crash reports. |
| `src/interface_packs.rs` | External interface language packs. |
| `src/ui/` | The egui views: tabs, wiki, About, backups, achievements, problem reports. |
| `src/app.rs`, `src/app/` | Application shell, configuration, history, file operations and windows. |

### Updating for a new game version

The catalogue and schemas are runtime views of the selected installation, not
hand-maintained game-data tables. Update readers in `src/game_data/` when the
game changes its assembly or prefab layout; unsupported data must be reported
as unavailable rather than replaced with older game constants. Then set
`SUPPORTED_GAME_VERSION` in
`src/game_info.rs` to the new version and run the tests against the game (see
below).

### Testing

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`tests/real_save.rs` round-trips `Exemple/save.sv` if it is present. To also
check item stats, the game version and the settings keys against an installed
game, set `CU_GAME_DIR` to the game folder before running `cargo test`. Tests
that need the game or a save are skipped otherwise.

### CI/CD and releases

- [`ci.yml`](.github/workflows/ci.yml) checks formatting, then runs Clippy
  (warnings are errors), the tests and a release build on Windows, Linux and
  macOS for every push to `main` and every pull request. The executables are
  uploaded as artifacts.
- [`release.yml`](.github/workflows/release.yml) runs when a `v*` tag is pushed.
  It checks that the tag matches the version in `Cargo.toml`, builds and
  packages the enabled platforms in its build matrix with build provenance attestations, and publishes
  a GitHub release with generated notes:

  ```sh
  git tag v0.2.0
  git push origin v0.2.0
  ```

- Public release executables are submitted through
  [`crazy-max/ghaction-virustotal`](https://github.com/crazy-max/ghaction-virustotal).
  Configure a repository Actions secret named `VT_API_KEY` before creating a
  release tag. The publish job requires the secret, submits one executable per
  downloaded release archive (not saves or game files), checks the matching
  number of analysis links, limits submissions to one
  per minute to allow for large-file API requests, and adds analysis links to
  the release notes and job summary.
  Submission does **not** mean an analysis has completed or that a file is safe;
  inspect the linked reports. VirusTotal shares submitted files with its
  community. Ordinary pushes and pull requests do not submit files.
- Actions use version tags, and [Dependabot](.github/dependabot.yml)
  opens weekly pull requests to keep them and the Cargo dependencies up to date.

## License

The editor's own code is licensed under [MIT](LICENSE).

This is an unofficial fan-made tool. It is not affiliated with or endorsed by
Orsoniks, Oro Interactive or Valve. *Casualties: Unknown* and its names, texts
and assets belong to their respective owners. The repository and the release
builds contain no game files: names, descriptions, item stats and effects are
read at run time from the copy of the game installed on your computer. You need
your own copy of the game, available on
[Steam](https://store.steampowered.com/app/4576490/).
