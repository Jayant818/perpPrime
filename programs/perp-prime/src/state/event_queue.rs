use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

pub const EVENT_SIZE: usize = 112;

/// reserved size after tag = EVENT_SIZE - 1 = 111
pub const ANY_RESERVED_A: usize = 64;
pub const ANY_RESERVED_B: usize = 32;
pub const ANY_RESERVED_C: usize = 15;
const _: () = assert!(ANY_RESERVED_A + ANY_RESERVED_B + ANY_RESERVED_C == EVENT_SIZE - 1);

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct AnyEvent {
    pub tag: u8,
    // split reserved into smaller arrays to satisfy derive checks
    pub reserved_a: [u8; ANY_RESERVED_A],
    pub reserved_b: [u8; ANY_RESERVED_B],
    pub reserved_c: [u8; ANY_RESERVED_C],
}
const _: () = assert!(std::mem::size_of::<AnyEvent>() == EVENT_SIZE);

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct FillEventPod {
    pub taker: [u8; 32],
    pub maker: [u8; 32],
    pub order_id: u128,
    pub price: u64,
    pub quantity: u64,
    pub taker_side: u8,     // 0 = Bid, 1 = Ask
    pub padding: [u8; 15],  // pads to EVENT_SIZE
}
const _: () = assert!(std::mem::size_of::<FillEventPod>() == EVENT_SIZE);

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct CancelEventPod {
    pub order_id: u64,
    pub owner: [u8; 32],
    pub quantity: u64,
    pub padding: [u8; 64], // pads to EVENT_SIZE
}
const _: () = assert!(std::mem::size_of::<CancelEventPod>() == EVENT_SIZE);

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct LiquidatePod {
    pub liquidator: [u8; 32],
    pub liquidated: [u8; 32],
    pub position_size: u64,
    pub price: u64,
    pub padding: [u8; 32],
}
const _: () = assert!(std::mem::size_of::<LiquidatePod>() == EVENT_SIZE);

/// High-level queue entry combining timestamp and payload tag+slot.
/// We keep timestamp separate from the POD slot so the slot remains uniform.
/// Note: this struct is NOT Pod because of the enum; keep it for higher-level logic only.
#[repr(C)]
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct EventQueueEntry {
    /// Unix timestamp (i64)
    pub timestamp: i64,
    /// The raw event slot bytes (length EVENT_SIZE). Use AnyEvent/FillEventPod/etc. to interpret.
    pub raw: [u8; EVENT_SIZE],
}

impl Default for EventQueueEntry {
    fn default() -> Self {
        Self {
            timestamp: 0,
            raw: [0u8; EVENT_SIZE],
        }
    }
}

/// Small helpers to convert between `Pubkey` and `[u8;32]`.
/// Note: cannot implement `From<Pubkey> for [u8; 32]` (or vice versa) due to the orphan rules,
/// so provide local conversion functions instead.
pub fn pubkey_to_array(pk: Pubkey) -> [u8; 32] {
    pk.to_bytes()
}

pub fn array_to_pubkey(bytes: [u8; 32]) -> Pubkey {
    Pubkey::new_from_array(bytes)
}
