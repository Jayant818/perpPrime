use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct UserAccount{
    pub owner : Pubkey,
    pub total_collateral: u64,
    pub available_collateral: u64,
    pub locked_collateral : u64,
    pub collateral_vault: Pubkey,
    #[max_len(20)]
    pub positions: Vec<Pubkey>, 
    pub bump: u8,
}