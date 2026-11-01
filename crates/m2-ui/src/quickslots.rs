//! Shared quick-slot actions.
//!
//! The client's `TQuickSlot.Type` is a `SLOT_TYPE_*` value: an item
//! (`SLOT_TYPE_INVENTORY`), a skill (`SLOT_TYPE_SKILL`) or an emotion
//! (`SLOT_TYPE_EMOTION`). `inventory.rs` owns the slots and their item
//! rendering; the skill and emotion pages own their icon overlays.

use bevy::prelude::*;
use m2_game::net::Net;
use m2_net::wire::{self, TQuickSlot, cg};

pub const ITEM: u8 = 1;
pub const SKILL: u8 = 2;
pub const EMOTION: u8 = 3;

/// A quick-slot key/click on a slot that is not an item.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickSlotUse {
    Skill(u32),
    Emotion(u32),
}

/// `SendQuickSlotAddPacket`: bind a skill/emotion to a global quick slot.
pub fn bind(net: &mut Net, slot: u8, kind: u8, position: u8) -> bool {
    slot < 32
        && matches!(kind, SKILL | EMOTION)
        && net.game.stream.send_packet(&wire::TPacketCGQuickSlotAdd {
            header: cg::QUICKSLOT_ADD,
            pos: slot,
            slot: TQuickSlot { Type: kind, Position: position },
        })
}

pub fn clear(net: &mut Net, slot: u8) -> bool {
    slot < 32
        && net.game.stream.send_packet(&wire::TPacketCGQuickSlotDel {
            header: cg::QUICKSLOT_DEL,
            pos: slot,
        })
}
