use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct UserPosition{
    pub market: Pubkey,
    pub user_account:Pubkey,
    pub quantity:u64,
    pub collateral:u64,
    pub bump:u8,
}