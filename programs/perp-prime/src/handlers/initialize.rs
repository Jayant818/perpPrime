use anchor_lang::prelude::*;
use anchor_spl::{ token_interface::{Mint, TokenAccount, TokenInterface}};
use crate::{GlobalConfig};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer:Signer<'info>,

    #[account(
        init,
        seeds = [
            b"config",
        ],
        bump,
        payer = payer,
        space = GlobalConfig::INIT_SPACE + GlobalConfig::DISCRIMINATOR.len()
    )]
    pub global_config : Account<'info,GlobalConfig>,

    #[account(
        init,
        seeds = [
            b"vault",
        ],
        bump,
        payer = payer,
        token::mint = vault_mint,
        token::authority = global_config,
    )]
    pub vault : InterfaceAccount<'info,TokenAccount>,

    #[account(
        init,
        seeds = [
            b"insurance_fund",
        ],
        bump,
        payer = payer,
        token::mint = vault_mint,
        token::authority = global_config
    )]
    pub insurance_fund: InterfaceAccount<'info,TokenAccount>,

    pub vault_mint : InterfaceAccount<'info, Mint>,

    pub token_program : Interface<'info,TokenInterface>,

    pub system_program : Program<'info, System>,
}

pub fn initialize(ctx: Context<Initialize>,step_size:u8,fee_rate:u8 ) -> Result<()> {

    let global_config = &mut ctx.accounts.global_config;
    let vault: &InterfaceAccount<'_, TokenAccount> = &ctx.accounts.vault;
    let vault_mint = &ctx.accounts.vault_mint;
    let insurance_fund = &ctx.accounts.insurance_fund;

    global_config.admin = ctx.accounts.payer.key();
    global_config.vault = vault.key();
    global_config.vault_mint = vault_mint.key();
    global_config.liquidation_fee_rate = fee_rate;
    global_config.step_size = step_size;
    global_config.insurance_fund = insurance_fund.key();
    global_config.config_bump = ctx.bumps.global_config;
    global_config.vault_bump = ctx.bumps.vault;
    global_config.insurance_fund_bump = ctx.bumps.insurance_fund;

    Ok(())
}
