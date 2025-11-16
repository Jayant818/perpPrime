use anchor_lang::prelude::*;

use crate::OpenOrdersAccount;

#[derive(Accounts)]
pub struct CreateOpenOrdersAccount<'info>{
    #[account(mut)]
    pub signer : Signer<'info>,

    #[account(
        init,
        space = OpenOrdersAccount::DISCRIMINATOR.len() +OpenOrdersAccount::INIT_SPACE,
        seeds = [
            b"open_orders",
            owner.key().as_ref(),
            market.key().as_ref()
        ],
        bump,
        payer = signer,
    )]
    pub open_orders_account: Account<'info,OpenOrdersAccount>,

    #[account(
        seeds = [   
            b"market",
            market.base_mint.as_ref(),
            market.quote_mint.as_ref(),
        ],
        bump = market.bump,
    )]
    pub market:Account<'info,Market>,

    pub system_program : Program<'info,System>,
}   

pub fn create_open_orders_account(ctx: Context<CreateOpenOrdersAccount>) -> Result<()> {
    let open_orders_account = &mut ctx.accounts.open_orders_account;

    open_orders_account.owner = ctx.accounts.signer.key();
    
    open_orders_account.market = ctx.accounts.market.key();
    
    open_orders_account.bump = ctx.bumps.open_orders_account;
    
    Ok(())
}