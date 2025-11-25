use std::slice;

use anchor_lang::prelude::*;
use bytemuck::{Zeroable,Pod};

use crate::{QueueHeader,error::ErrorCode};


#[account(zero_copy)]
#[repr(C)]
pub struct RequestQueueAccount{
    pub header:QueueHeader,
    // flexible array member, keeping it non-pub so that we don't access it directly
    entries:[RequestItem;0]
}

impl RequestQueueAccount {
    fn get_entries(&self)-> &[RequestItem] {
        // We are getting a pointer to self and offsetting it past the header.
        let entries_ptr = unsafe {
            // *const self - raw pointer to the struct 
            // *const u8 - reintrepreting the pointer as the byte pointer
            (self as *const Self as *const u8).add(std::mem::size_of::<QueueHeader>())
        } as *const RequestItem;

        // we use capacity from the header to create a valid slice
        let capacity = self.header.capacity as usize;
        unsafe{
            slice::from_raw_parts(entries_ptr,capacity)
        }
    }

    fn get_entries_mut(&mut self)-> &mut [RequestItem]{
        // get mut ptr for entries 
        let entries_ptr = unsafe {
            (self as *const Self as *const u8).add(std::mem::size_of::<QueueHeader>())
        } as *mut RequestItem;

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

    pub fn push(&mut self,item:&RequestItem)->Result<()>{

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

    pub fn peek(&self)-> Result<Option<RequestItem>>{
        if self.header.count == 0 {
            return Ok(None)
        }

        let slot_index = self.header.tail as usize;

        let item = self.get_entries()[slot_index];

        return Ok(Some(item))
    }

}

#[derive(Clone,Copy,AnchorDeserialize,AnchorSerialize,Default,Debug,InitSpace)]
#[repr(C)]
pub enum OrderType{
    #[default]
    MarketOrder,
    LimitOrder
}

#[repr(C)]
#[derive(Clone,Copy,Debug,AnchorDeserialize,AnchorSerialize,Default,InitSpace)]
pub enum OrderSide{
    #[default]
    BID = 0,
    ASK = 1
}

#[repr(C)]
#[derive(Clone,Copy,AnchorDeserialize,AnchorSerialize,InitSpace,Default,Debug)]
pub enum OrderPosition{
    #[default]
    SHORT = 0,
    LONG = 1
}

#[repr(C)]
#[derive(Clone,Copy,AnchorDeserialize,AnchorSerialize,Default)]
pub enum RequestType{
    #[default]
    OPEN = 0,
    CANCEL = 1,
    LIQUIDATION = 2,
}

#[repr(C)]
#[derive(Clone, Copy, Debug,Pod, Zeroable)]
pub struct RequestItem{
    pub request_type:u8, // 0 = OPEN , 1 = CANCEL
    pub order_type: u8,  // 0 = Market Order, 1 = limit Order
    pub order_side :u8, // 0 = Bid, 1 = Ask
    pub position: u8,   // 0 = SHORT , 1 = LONG
    pub padding0:[u8;4],
    pub quantity: u64,
    pub user: Pubkey,
    pub order_id: u128,
    // pub padding1: [u8; 0],
    // pub entry_price: u64, - Not suitable for Cancel Order, instead we show the order_id
}

impl RequestItem {
    pub fn init(
    request_type:u8,
    order_type: u8,
    order_side :u8, 
    position: u8,   
    quantity: u64,
    user: Pubkey,
    order_id: u128,
    )->Self{
        Self { 
            request_type, 
            order_type, 
            order_side, 
            position, 
            padding0:[0;4], 
            quantity, 
            user, 
            order_id }            
    }
}