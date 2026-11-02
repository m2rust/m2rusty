//! Metin2 data formats, independent of Bevy.
//!
//! Each module is a port of a C++ reader and is tested against it (see
//! `tests/differential_*.rs`). Names and contents are bytes: the client's data
//! is CP949/CP1252, not UTF-8.

pub mod attribute;
pub mod crypt;
pub mod effect;
pub mod gr2;
pub mod image;
pub mod locale;
pub mod map;
pub mod pack;
pub mod property;
pub mod proto;
pub mod race;
pub mod script;
pub mod spt;
pub mod text;
pub mod utf8;
