use anchor_lang::prelude::*;

use crate::OrderSide;

#[repr(C)]
#[derive(AnchorDeserialize,AnchorSerialize,Clone,Debug,Default)]
pub struct EventQueue{
    pub event_type:EventType,
    pub timestamp:i64,
}

#[repr(C)]
#[derive(Debug,Clone,AnchorDeserialize,AnchorSerialize)]
pub enum EventType{
    Fill(FillEvent),
    Cancel(CancelEvent),
    Liquidate(Liquidate)
}

impl Default for EventType{
    fn default() -> Self {
        EventType::Fill(FillEvent::default())    
    }
}

#[repr(C)]
#[derive(Debug,Clone,AnchorDeserialize,AnchorSerialize,Default)]
pub struct FillEvent{
    pub taker:Pubkey,
    pub maker:Pubkey,
    pub order_id:u64,
    pub price : u64,
    pub quantity: u64,
    pub taker_side: OrderSide,
}

#[repr(C)]
#[derive(Debug,Clone,AnchorDeserialize,AnchorSerialize,Default)]
pub struct CancelEvent{
    pub order_id: u64,
    pub owner: Pubkey,
    pub quantity: u64,
}

#[repr(C)]
#[derive(Debug,Clone,AnchorDeserialize,AnchorSerialize,Default)]
pub struct Liquidate{
    pub liquidator: Pubkey,
    pub liquidated: Pubkey,
    pub position_size: u64,
    pub price:u64,
}
