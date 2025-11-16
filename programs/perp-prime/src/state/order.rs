use anchor_lang::prelude::*;

use crate::{OrderPosition, OrderSide,OrderType};

#[derive(Clone,Copy,Debug,InitSpace,AnchorDeserialize,AnchorSerialize,Default,PartialEq)]
pub enum OrderStatus{
    #[default]
    FREE, // -> the initial state of all nodes
    PENDING, // -> sent to the request_queue, not yet on the book
    OPEN,  // on the slab
    FILLED, // completely filled
    CANCELLED, // sent to the request queue, not yet removed from book
}

#[derive(Clone, Copy,Debug,AnchorDeserialize,AnchorSerialize,InitSpace)]
#[repr(C)]
pub struct Order{
    pub status: OrderStatus,
    pub quantity: u64,
    pub order_id :u128,
    // Order Id provided by the client for easier lookup,
    pub client_order_id:u64,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub position:OrderPosition,
    pub locked_margin: u64,
    pub entry_price:u64,
}
