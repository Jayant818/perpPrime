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
    pub is_liquidating:bool,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub position:OrderPosition,
    pub locked_margin: u64,
    pub limit_price:u64,
}


impl Order {
    pub fn new(
        status: OrderStatus,
        quantity: u64,
        order_id :u128,
        client_order_id:u64,
        is_liquidating:bool,
        side: OrderSide,
        order_type: OrderType,
        position:OrderPosition,
        locked_margin: u64,
        limit_price:u64,
    )->Self{
        Self { 
            status, 
            quantity, 
            order_id, 
            client_order_id, 
            is_liquidating, 
            side, 
            order_type, 
            position, 
            locked_margin, 
            limit_price 
        }
    }
}