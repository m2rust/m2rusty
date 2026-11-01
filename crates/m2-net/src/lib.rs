//! Metin2 client networking without Bevy (docs/bevy-port-plan.md, phase 4).
//!
//! Sans-IO: `wire` (packets), `stream` (framing), `phase` (state machine),
//! `transport` (byte pipe trait and a `std::net` adapter).
pub mod wire;
