use anchor_lang::prelude::*;
use anchor_spl::{
    token_interface::{Mint, TokenAccount, TokenInterface, transfer_checked, TransferChecked},
    associated_token::AssociatedToken,
};
use crate::{GlobalConfig, UserAccount, error::ErrorCode};

#[derive(Accounts)]
pub struct DepositCollateral<'info> {
    #[account(mut)]
    pub owner: Signer<'info>, 

    #[account(
        init_if_needed,
        payer = owner,
        space = 8 + UserAccount::INIT_SPACE,
        seeds = [b"user_account", owner.key().as_ref()],
        bump,
    )]
    pub user_account: Account<'info, UserAccount>,

    #[account(
        seeds = [b"config"],
        bump = global_config.config_bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init_if_needed,
        payer = owner,
        token::mint = token_mint,
        token::authority = user_account, // The user_account is the authority
        seeds = [b"user_vault", user_account.key().as_ref()],
        bump,
    )]
    pub user_collateral_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        address = global_config.vault_mint @ ErrorCode::VaultMintMismatch,
    )]
    pub token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = owner,
    )]
    pub user_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn deposit_collateral(ctx: Context<DepositCollateral>, amount: u64) -> Result<()> {
    let user_account = &mut ctx.accounts.user_account;

    if user_account.owner == Pubkey::default() {
        user_account.owner = ctx.accounts.owner.key();
        user_account.collateral_vault = ctx.accounts.user_collateral_vault.key();
        user_account.total_collateral = 0;
        user_account.available_collateral = 0;
        user_account.locked_collateral = 0;
        user_account.bump = ctx.bumps.user_account;
    }

    let transfer_accounts = TransferChecked {
        authority: ctx.accounts.owner.to_account_info(),
        mint: ctx.accounts.token_mint.to_account_info(),
        from: ctx.accounts.user_ata.to_account_info(),
        to: ctx.accounts.user_collateral_vault.to_account_info(),
    };
    let cpi_context = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        transfer_accounts
    );
    transfer_checked(cpi_context, amount, ctx.accounts.token_mint.decimals)?;

    user_account.total_collateral = user_account.total_collateral
        .checked_add(amount)
        .ok_or(ErrorCode::AdditionOverflow)?;
    user_account.available_collateral = user_account.available_collateral
        .checked_add(amount)
        .ok_or(ErrorCode::AdditionOverflow)?;

    Ok(())
}