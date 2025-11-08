use anchor_lang::prelude::*;

#[account]
pub struct User{
    owner : Pubkey,
    owner_vault: Pubkey,
    
}