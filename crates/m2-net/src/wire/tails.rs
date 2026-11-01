//! Parsers for the variable-length tails of dynamic-size packets that the C++ handlers
//! read after the fixed struct. Other dynamic packets (guild, messenger, shop, quest
//! info, NPC position, land list, dungeon) keep their raw `tail` bytes; their
//! sub-formats are consumed by the game-side handlers, not by the network layer.

use super::{DecodeError, TPacketGCSyncPositionElement, Wire, cstr_to_string};

/// `RecvChatPacket`, `RecvWhisperPacket`, `RecvScriptPacket`: the tail is text,
/// NUL-terminated by the sender (the handlers force a final NUL).
pub fn text(tail: &[u8]) -> String {
    cstr_to_string(tail)
}

/// `RecvSyncPositionPacket`: `(wSize - sizeof(header)) / sizeof(element)` elements.
/// A partial trailing element is ignored (as the integer division does).
pub fn sync_positions(tail: &[u8]) -> Result<Vec<TPacketGCSyncPositionElement>, DecodeError> {
    tail.chunks_exact(TPacketGCSyncPositionElement::SIZE).map(TPacketGCSyncPositionElement::decode).collect()
}

/// `RecvDuelStartPacket`: a list of VIDs, `(wSize - sizeof(packet)) / 4` of them.
pub fn duel_vids(tail: &[u8]) -> Vec<u32> {
    tail.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}
