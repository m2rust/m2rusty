//! Metin2 data formats, independent of Bevy.
//!
//! Each module is a port of a C++ reader and is tested against it (see
//! `tests/differential.rs`). Names and contents are bytes: the client's data
//! is CP949/CP1252, not UTF-8.

pub mod map;
pub mod pack;
pub mod script;
pub mod text;
