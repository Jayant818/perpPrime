use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct DepositCollateral{

}

pub fn deposit_collateral(ctx:context<DepositCollateral>)->Result<()>{
    Ok(())
}