//! Metin2 client networking without Bevy (docs/bevy-port-plan.md, phase 4).
//!
//! Sans-IO: `wire` (generated packet structs and codecs), `stream` (buffers, framing,
//! sequence byte), `phase` (state machine producing `NetEvent`s), `transport` (byte pipe
//! trait, in-memory pair, `std::net` adapter behind feature `tcp`).
//!
//! There is no stream cipher in the C++ wire protocol (see `stream`). Regenerate the wire
//! layer with `perl tools/differential/net/gen_packets.pl` and `gen_seq.pl` after changing
//! Packet.h.
#![allow(clippy::field_reassign_with_default, clippy::large_enum_variant, clippy::chunks_exact_to_as_chunks, clippy::too_many_arguments)]

pub mod phase;
pub mod stream;
pub mod transport;
pub mod wire;
