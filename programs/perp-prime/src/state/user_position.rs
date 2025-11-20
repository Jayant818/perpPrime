use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct UserPosition{
    pub market: Pubkey,
    pub owner:Pubkey,
    pub quantity:i64,
    pub collateral:u64,
    pub last_cumulative_funding_rate:i128,
    pub bump:u8,
}