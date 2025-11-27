use anchor_lang::prelude::*;

use crate::{Market, OpenOrdersAccount};

#[derive(Accounts)]
pub struct CreateOpenOrdersAccount<'info>{
    #[account(mut)]
    pub signer : Signer<'info>,

    #[account(
        init,
        space = OpenOrdersAccount::DISCRIMINATOR.len() + std::mem::size_of::<OpenOrdersAccount>(),
        seeds = [
            b"open_orders",
            signer.key().as_ref(),
            market.key().as_ref()
        ],
        bump,
        payer = signer,
    )]
    pub open_orders_account: AccountLoader<'info,OpenOrdersAccount>,

    #[account(
        seeds = [   
            b"market",
            market.base_mint.as_ref(),
            market.quote_mint.as_ref(),
        ],
        bump = market.market_bump,
    )]
    pub market:Account<'info,Market>,

    pub system_program : Program<'info,System>,
}   

pub fn create_open_orders_account(ctx: Context<CreateOpenOrdersAccount>) -> Result<()> {
    let mut open_orders_account=  ctx.accounts.open_orders_account.load_mut()?;

    open_orders_account.owner = ctx.accounts.signer.key();
    
    open_orders_account.market = ctx.accounts.market.key();
    
    open_orders_account.bump = ctx.bumps.open_orders_account;
    
    Ok(())
}