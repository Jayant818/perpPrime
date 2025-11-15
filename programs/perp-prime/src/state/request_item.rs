use anchor_lang::prelude::*;
use bytemuck::{Zeroable,Pod};

#[derive(Clone,Copy,AnchorDeserialize,AnchorSerialize,Default)]
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
#[derive(Clone,AnchorDeserialize,AnchorSerialize,InitSpace,Default)]
pub enum OrderPosition{
    #[default]
    SHORT = 0,
    LONG = 1
}

#[repr(C)]
#[derive(Clone,Copy,AnchorDeserialize,AnchorSerialize,Default)]
pub enum RequestType{
    #[default]
    OPEN,
    CANCEL
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
    pub padding1: [u8; 0],
    // pub entry_price: u64, - Not suitable for Cancel Order, instead we show the order_id
}