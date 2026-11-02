//! Parsers for the variable-length tails of dynamic-size packets that the C++ handlers
//! read after the fixed struct. Other dynamic packets (guild, messenger, shop, quest
//! info, NPC position, land list, dungeon) keep their raw `tail` bytes; their
//! sub-formats are consumed by the game-side handlers, not by the network layer.

use super::{DecodeError, Reader, TPacketGCSyncPositionElement, Wire, cstr_to_string};

/// The optional fields of `RecvQuestInfoPacket`, in flag order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestInfo {
    pub begin: Option<bool>,
    pub title: Option<String>,
    pub clock_name: Option<String>,
    pub clock_value: Option<i32>,
    pub counter_name: Option<String>,
    pub counter_value: Option<i32>,
    pub icon: Option<String>,
}

pub fn quest_info(flags: u8, tail: &[u8]) -> Result<QuestInfo, DecodeError> {
    let mut reader = Reader::new(tail);
    let begin = if flags & 1 != 0 { Some(reader.take(1)?[0] != 0) } else { None };
    let title = if flags & 2 != 0 { Some(cstr_to_string(reader.take(31)?)) } else { None };
    let clock_name = if flags & 4 != 0 { Some(cstr_to_string(reader.take(17)?)) } else { None };
    let clock_value = if flags & 8 != 0 { Some(i32::read(&mut reader)?) } else { None };
    let counter_name = if flags & 16 != 0 { Some(cstr_to_string(reader.take(17)?)) } else { None };
    let counter_value = if flags & 32 != 0 { Some(i32::read(&mut reader)?) } else { None };
    let icon = if flags & 64 != 0 { Some(cstr_to_string(reader.take(25)?)) } else { None };
    Ok(QuestInfo { begin, title, clock_name, clock_value, counter_name, counter_value, icon })
}

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
