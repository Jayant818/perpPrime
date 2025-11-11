use anchor_lang::prelude::*;

#[derive(InitSpace)]
#[account]
pub struct User{
    pub owner : Pubkey,
    pub collateral_balance: u64,
    #[max_len(20)]
    pub positions: Vec<Pubkey>, 
    pub bump: u8,
}

impl User{
    pub fn init_if_needed(&mut self,owner:Pubkey){
        if self.owner == Pubkey::default() {
            self.owner = owner;
            self.collateral_balance = 0;
        }
    }
}