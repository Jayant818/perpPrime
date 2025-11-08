use anchor_lang::prelude::*;

#[derive(InitSpace)]
#[account]
pub struct GlobalConfig{
    pub vault : Pubkey,
    pub vault_mint : Pubkey,
    pub insurance_fund : Pubkey,
    pub fee_rate : u8,
    pub step_size: u8,
    pub admin: Pubkey,
}