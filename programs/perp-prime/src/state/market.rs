use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Market{
    pub bids: Pubkey,
    pub asks:Pubkey,
    pub request_queue: Pubkey,
    pub event_queue: Pubkey,
    pub oracle_price_feed: Pubkey,
    pub funding_rate : i64, // currrent funding rate - scaled by 1e6
    pub funding_clamp:i64, // scaled 1e6
    pub last_funding_time:u128,
    pub funding_interval:u8,
    pub open_interest: u128, 
    pub cummulative_funding_rate : i128,
    pub initial_margin_rate: u64,
    pub maintainence_margin: u64,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub base_lot_size: u64,
    pub quote_lot_size: u64,
    pub sequence: u64,
    pub bump : u8,
    pub bids_bump: u8,
    pub asks_bump: u8,
    pub event_queue_bump: u8,
    pub request_queue_bump: u8,
}