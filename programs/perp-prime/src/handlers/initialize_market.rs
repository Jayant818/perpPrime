
use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

use crate::{CircularQueue, EventQueue, GlobalConfig, Market, QueueHeader, RequestQueue, Slab};

#[derive(Accounts)]
#[instruction(pair:String)]
pub struct InitializeMarket<'info>{
    #[account(
        mut,
    )]
    pub signer:Signer<'info>,

    #[account(
        init,
        payer = signer,
        seeds = [
            b"market",
            pair.as_bytes(),
        ],
        bump,
        space =  Market::INIT_SPACE + Market::DISCRIMINATOR.len()
    )]
    pub market: Account<'info,Market>,

    #[account(
        seeds = [
             b"config"
        ],
        bump = config.config_bump
    )]
    pub config: Account<'info,GlobalConfig>,

    pub base_mint: InterfaceAccount<'info,Mint>,

    pub quote_mint: InterfaceAccount<'info,Mint>,

    #[account(
        init,
        payer = signer,
        space = 8 + std::mem::size_of::<QueueHeader>() + std::mem::size_of::<T>(),
        seeds  = [
            b"request_queue",
            base_mint.key().as_ref(),
            quote_mint.key().as_ref()
        ],
        bump,
    )]
    /// CHECK;
    pub request_queue: UncheckedAccount<'info>,

    #[account(
        init,
        payer = signer,
        space = 8 +  std::mem::size_of::<QueueHeader>() + std::mem::size_of::<T>(),
        seeds = [
            b"event_queue",
            base_mint.key().as_ref(),
            quote_mint.key().as_ref()
        ],
        bump,
    )]
    /// CHECK:
    pub event_queue: UncheckedAccount<'info>,

    #[account(
        init,
        payer = signer,
        space = 8,
        seeds = [
            b"bids",
            base_mint.key().as_ref(),
            quote_mint.key().as_ref(),
        ],
        bump,
    )]
    pub bids : UncheckedAccount<'info>,

    #[account(
        init,
        payer = signer,
        space = 8,
        seeds = [
            b"asks",
            base_mint.key().as_ref(),
            quote_mint.key().as_ref(),
        ],
        bump,
    )]
    pub asks : UncheckedAccount<'info>,

    pub system_program : Program<'info , System>,

}

pub fn initialize_market(ctx:Context<InitializeMarket>,pair:String)->Result<()>{

    let mut event_queue = ctx.accounts.event_queue.try_borrow_mut_data()?;
    let mut request_queue = ctx.accounts.request_queue.try_borrow_mut_data()?;

    let mut bids_slab = ctx.accounts.bids.try_borrow_mut_data()?;
    let mut asks_slab = ctx.accounts.asks.try_borrow_mut_data()?;
    

    CircularQueue::<RequestQueue>::intialize(&mut request_queue, 128)?;

    CircularQueue::<EventQueue>::intialize(&mut event_queue, 256)?;

    Slab::initialize(&mut bids_slab)?;
    Slab::initialize(&mut asks_slab)?;

    let market = &mut ctx.accounts.market;

    market.sequence = 0;


    Ok(())
}