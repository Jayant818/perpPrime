use anchor_lang::prelude::*;
use anchor_spl::{token::TokenAccount, token_interface::{TokenInterface, TransferChecked,transfer_checked}};

use crate::{GlobalConfig, UserAccount,error::ErrorCode};


#[derive(Accounts)]
// I think here we need to lock the user 
pub struct WithdrawCollateral<'info>{
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        associated_token::authority = user,
        associated_token::mint = config.vault_mint,
    )]
    pub user_ata: InterfaceAccount<'info,TokenAccount>,

    #[account(
        seeds = [
            b"user_account",
            user.key().as_ref()
        ],
        bump = user_account.bump,
    )]
    pub user_account:Account<'info,UserAccount>,

    #[account(
        seeds = [
             b"config"
        ],
        bump = config.config_bump
    )]
    pub config: Account<'info,GlobalConfig>,

    #[account(
        mut,
        seeds = [b"user_vault", user_account.key().as_ref()],
        bump,

    )]
    pub user_collateral_vault : InterfaceAccount<'info,TokenAccount>,

    pub token_program: Interface<'info,TokenInterface>,
}

pub fn withdraw_collateral(ctx:Context<WithdrawCollateral>, amount_to_withdraw: u64)->Result<()>{

    let user_account = &ctx.accounts.user_account;

    require!(user_account.available_collateral >= amount_to_withdraw,ErrorCode::InsufficientCollateral);

    let user_ata = &mut ctx.accounts.user_ata;

    let collateral_account = &mut ctx.accounts.user_collateral_vault;

    let token_program = &ctx.accounts.token_program;

    let config = &ctx.accounts.config;

    let transfer_accounts = TransferChecked{
        authority:user_account.to_account_info(),
        from: collateral_account.to_account_info(),
        mint: token_program.to_account_info(),
        to: user_ata.to_account_info(),
    };

    let cpi_context = CpiContext::new(
        token_program.to_account_info(), 
        transfer_accounts
    );

    transfer_checked(cpi_context, amount_to_withdraw, config.decimals)?;

    user_account.available_collateral.checked_sub(amount_to_withdraw).ok_or(ErrorCode::SubtractionUnderFlow)?;
    user_account.total_collateral.checked_sub(amount_to_withdraw).ok_or(ErrorCode::SubtractionUnderFlow)?;

}

