use anchor_lang::prelude::*;

#[derive(InitSpace)]
#[account]
pub struct GlobalConfig{
    pub vault : Pubkey,
    pub vault_mint : Pubkey,
    pub insurance_fund : Pubkey,
    pub liquidation_fee_rate : u8,
    pub step_size: u8,
    pub admin: Pubkey,
    pub config_bump:u8,
    pub vault_bump:u8,
    pub insurance_fund_bump:u8,
    pub max_leverage:u8,
    pub pause:bool,
}