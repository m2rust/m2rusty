//! Metin2 data formats, independent of Bevy.
//!
//! Each module is a port of a C++ reader and is tested against it (see
//! `tests/differential_*.rs`). Names and contents are bytes: the client's data
//! is CP949/CP1252, not UTF-8.

pub mod crypt;
pub mod locale;
pub mod pack;
pub mod proto;
pub mod script;
pub mod spt;
pub mod text;
