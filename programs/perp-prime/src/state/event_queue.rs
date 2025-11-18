use std::slice::from_raw_parts_mut;

use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

use crate::{QueueHeader, EventQueueEntry};

pub const EVENT_SIZE: usize = 112;

/// reserved size after tag = EVENT_SIZE - 1 = 111
pub const ANY_RESERVED_A: usize = 64;
pub const ANY_RESERVED_B: usize = 32;
pub const ANY_RESERVED_C: usize = 15;
const _: () = assert!(ANY_RESERVED_A + ANY_RESERVED_B + ANY_RESERVED_C == EVENT_SIZE - 1);

pub const FILL_EVENT: u8 = 0;
pub const CANCEL_EVENT:u8 = 1;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct AnyEvent {
    pub tag: u8,   // 0 - FIll , 1 - Cancel
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
    pub order_id: u128,
    pub owner: [u8; 32],
    pub quantity: u64,
    pub padding_a: [u8; 32],
    pub padding_b: [u8; 24],
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

#[repr(C)]
#[derive(Pod,Zeroable, Clone,Copy, Debug)]
pub struct EventQueueEntry {
    /// Unix timestamp (i64)
    pub timestamp: i64,
    /// The raw event slot bytes (length EVENT_SIZE). Use AnyEvent/FillEventPod/etc. to interpret.
    pub raw: AnyEvent,
}

impl Default for EventQueueEntry {
    fn default() -> Self {
        Self {
            timestamp: 0,
            raw: AnyEvent::zeroed(),
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

#[repr(C)]
#[account(zero_copy)]
pub struct EventQueueAccount{
    pub header: QueueHeader,
    entries: [EventQueueEntry;0],
}

impl EventQueueAccount{
    fn get_entries(&self)-> &[EventQueueEntry] {
        // We are getting a pointer to self and offsetting it past the header.
        let entries_ptr = unsafe {
            // *const self - raw pointer to the struct 
            // *const u8 - reintrepreting the pointer as the byte pointer
            (self as *const Self as *const u8).add(std::mem::size_of::<QueueHeader>())
        } as *const EventQueueEntry;

        // we use capacity from the header to create a valid slice
        let capacity = self.header.capacity as usize;
        unsafe{
            slice::from_raw_parts(entries_ptr,capacity)
        }
    }

    fn get_entries_mut(&mut self)-> &mut [EventQueueEntry]{
        // get mut ptr for entries 
        let entries_ptr = unsafe {
            (self as *const Self as *const u8).add(std::mem::size_of::<QueueHeader>())
        } as *mut EventQueueEntry;

        let capacity = self.header.capacity as usize;

        unsafe {
            slice::from_raw_parts_mut(entries_ptr, capacity)
        }
    }

    pub fn initialize(&mut self, capacity: u64) {
        self.header.head = 0;
        self.header.count = 0;
        self.header.tail = 0;
        self.header.capacity = capacity;
    }

    pub fn push(&mut self,item:&EventQueueEntry)->Result<()>{

        require!(self.header.count < self.header.capacity, ErrorCode::QueueIsFull);
        
        let slot_index = self.header.tail as usize;

        let entries = self.get_entries_mut();
        entries[slot_index] = *item;
        self.header.tail = (self.header.tail+1) % self.header.capacity;
        self.header.count +=1;

        Ok(())
    }

    pub fn pop(&mut self)-> Result<Option<ResultItem>>{
        if self.header.count == 0 {
            return Ok(None)
        }

        let slot_index = self.header.tail as usize;

        // getting an immutable copy 
        let item_copy = self.get_entries()[slot_index];

        self.header.head = (self.header.head + 1)%self.header.capacity;
        self.header.count -=1;

        Ok(Some(item_copy))
    }

    pub fn peek(&self)-> Result<Option<EventQueueEntry>>{
        if self.header.count == 0 {
            return Ok(None)
        }

        let slot_index = self.header.tail as usize;

        let item = self.get_entries()[slot_index];

        return Ok(Some(item))
    }

}