//! # Casualties Unknown save editor
//!
//! A graphical editor for the save file of the game *Casualties Unknown*
//! (`save.sv`), built with [egui](https://github.com/emilk/egui).
//!
//! The crate is organised as follows:
//!
//! * [`save`] – reading/writing the gzip-compressed JSON save and backups,
//! * [`json`] – helpers for editing the untyped JSON document safely,
//! * [`model`] – structural edits that keep the document consistent,
//! * [`schema`] – labels, ranges and meaning of known fields,
//! * [`catalog`] – ids of items and liquids known to the game,
//! * [`locale`] – optional display names from the game's language files,
//! * [`game_info`] – installed game version and Steam build, compatibility,
//! * [`achievements`] – the game's Steam achievements from Steam's local cache,
//! * [`player_prefs`] – game progression outside the save (character unlocks,
//!   layers reached, endings),
//! * [`diagnostics`] – anonymised debugging information and crash reports for bug reports,
//! * [`platform`] – operating-system specific paths and helpers (save folder,
//!   Steam libraries, Proton prefixes, fonts),
//! * [`app`] and [`ui`] – the egui application.
//!
//! The editor never drops data it does not understand: the whole save is
//! kept as a JSON tree and only the fields the user touches are changed.

pub mod achievements;
pub mod app;
pub mod catalog;
pub mod diagnostics;
pub mod game_data;
pub mod game_info;
pub mod interface_packs;
pub mod json;
pub mod locale;
pub mod model;
pub mod platform;
pub mod player_prefs;
pub mod save;
pub mod schema;
pub mod ui;
