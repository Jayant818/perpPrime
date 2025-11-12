
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface, transfer_checked, TransferChecked};

use crate::{GlobalConfig, User, UserAccount, error::ErrorCode};

#[derive(Accounts)]
pub struct DepositCollateral<'info>{
    #[account(mut)]
    pub signer : Signer<'info>,

    #[account(
        init_if_needed,
        payer = signer,
        space = UserAccount::INIT_SPACE + UserAccount::DISCRIMINATOR.len(),
        seeds = [
            b"user_account",
            signer.key().as_ref()
        ],
        bump,
    )]
    pub user_account : Account<'info,UserAccount>,

    #[account(
        seeds = [
             b"config"
        ],
        bump = global_config.config_bump
    )]
    pub global_config : Account<'info,GlobalConfig>,

    #[account(
        mut,
        seeds = [
            b"vault"
        ],
        bump = global_config.vault_bump,
        constraint = program_vault.mint == global_config.vault_mint @ErrorCode::VaultMintMismatch,
    )]
    pub program_vault : InterfaceAccount<'info,TokenAccount>,

    #[account(
        constraint = token_mint.key() == global_config.vault_mint @ErrorCode::VaultMintMismatch,
    )]
    pub token_mint : InterfaceAccount<'info,Mint>,

    #[account(
        mut,
        constraint = user_ata.mint == global_config.vault_mint @ErrorCode::VaultMintMismatch,
    )]
    pub user_ata : InterfaceAccount<'info,TokenAccount>,

    pub token_program: Interface<'info,TokenInterface>,

    pub system_program : Program<'info,System>,

}

pub fn deposit_collateral(ctx:Context<DepositCollateral>,amount:u64,decimals:u8)->Result<()>{

    let user = &ctx.accounts.signer;
    let user_ata = & ctx.accounts.user_ata;
    let vault = & ctx.accounts.program_vault;
    let token_program = &ctx.accounts.token_program;
    let token_mint = &ctx.accounts.token_mint;
    let user_account = &mut ctx.accounts.user_account;

    let transfer_accounts = TransferChecked{
        authority: user.to_account_info(),
        mint:token_mint.to_account_info(),
        from: user_ata.to_account_info(),
        to:vault.to_account_info(),
    };

    let cpi_context = CpiContext::new(token_program.to_account_info(), transfer_accounts);

    transfer_checked(cpi_context, amount, decimals)?;


    user_account.init_if_needed(user.key());

    user_account.owner = user.key();
    user_account.collateral_balance = user_account.collateral_balance.checked_add(amount).ok_or(ErrorCode::AdditionOverflow)?;

    Ok(())
}