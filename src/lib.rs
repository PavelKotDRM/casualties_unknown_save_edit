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
//! * [`app`] and [`ui`] – the egui application.
//!
//! The editor never drops data it does not understand: the whole save is
//! kept as a JSON tree and only the fields the user touches are changed.

pub mod app;
pub mod catalog;
pub mod game_data;
pub mod interface_packs;
pub mod json;
pub mod locale;
pub mod model;
pub mod save;
pub mod schema;
pub mod ui;
