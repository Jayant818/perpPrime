use anchor_lang::prelude::*;
use crate::UserPosition;

#[derive(Accounts)]
pub struct InitializeUserPosition<'info>{
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        seeds = [
            b"user_position",
            payer.key().as_ref(),
            market.key().as_ref()
        ],
        bump,
        payer = payer,
        space = UserPosition::DISCRIMINATOR.len() + UserPosition::INIT_SPACE,
    )]
    pub user_position:Account<'info,UserPosition>,

    pub market: Account<'info,Market>,

    pub system_program: Program<'info,System>,
}

pub fn initialize_user_position(ctx:Context<InitializeUserPosition>)->Result<()>{
    let position = &mut ctx.accounts.user_position;
    position.owner = ctx.accounts.payer.key();
    position.market = ctx.accounts.market.key();
    position.quantity = 0;
    position.collateral = 0;
    position.last_cumulative_funding_rate = 0;
    position.bump = ctx.bumps.user_position;
    Ok(())
}