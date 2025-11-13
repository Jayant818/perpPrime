use anchor_lang::prelude::*;

use crate::{OrderPosition, OrderSide};

#[event]
pub struct OrderQueued{
    pub user: Pubkey,
    pub market: Pubkey,
    pub order_id:u128,
    pub quantity: u64,
    pub side:OrderSide,
    pub position:OrderPosition,
}