use anchor_lang::prelude::*;

use crate::{OrderPosition, OrderSide};

#[derive(Clone,Debug,InitSpace,AnchorDeserialize,AnchorSerialize)]
pub enum OrderStatus{
    PENDING,
    CANCELLED,
    EXECUTED
}

#[account]
#[derive(InitSpace)]
pub struct Order{
    pub user: Pubkey,
    pub status: OrderStatus,
    pub quantity: u64,
    pub side: OrderSide,
    pub position:OrderPosition,
    pub collateral: u64,
    pub entry_price:u64,
    pub bump:u8,
}
