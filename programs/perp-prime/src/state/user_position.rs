use anchor_lang::prelude::*;

#[derive(AnchorSerialize,AnchorDeSerialize,PartialEq)]
pub enum PositionStatus{
    Active = 0,
    Liquidating = 1,
}

#[account]
#[derive(InitSpace)]
pub struct UserPosition{
    pub market: Pubkey,
    pub owner:Pubkey,
    pub quantity:i64, // signed qty + = long , - = short
    pub collateral:u64,
    pub avg_entry_price:u64, // price * 1e6 - will be updated on every trade
    pub last_cumulative_funding_rate:i128,
    pub status:PositionStatus,
    pub bump:u8,
}