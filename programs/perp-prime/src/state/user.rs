use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct UserAccount{
    pub owner : Pubkey,
    pub collateral_balance: u64,
    #[max_len(20)]
    pub positions: Vec<Pubkey>, 
    pub locked_collateral : u64,
    pub bump: u8,
}

impl UserAccount{
    pub fn init_if_needed(&mut self,owner:Pubkey){
        if self.owner == Pubkey::default() {
            self.owner = owner;
            self.collateral_balance = 0;
        }
    }
}