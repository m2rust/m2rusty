//! Metin2 client networking without Bevy (docs/bevy-port-plan.md, phase 4).
//!
//! Sans-IO: `wire` (packets), `stream` (framing), `phase` (state machine),
//! `transport` (byte pipe trait and a `std::net` adapter).
#![allow(clippy::field_reassign_with_default, clippy::large_enum_variant, clippy::chunks_exact_to_as_chunks, clippy::too_many_arguments)]

pub mod phase;
pub mod stream;
pub mod transport;
pub mod wire;
