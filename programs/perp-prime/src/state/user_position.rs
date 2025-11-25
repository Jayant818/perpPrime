use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct UserPosition{
    pub market: Pubkey,
    pub owner:Pubkey,
    pub quantity:i64, // signed qty + = long , - = short
    pub collateral:u64,
    pub avg_entry_price:u64, // price * 1e6
    pub last_cumulative_funding_rate:i128,
    pub is_liquidating: bool,
    pub bump:u8,
}