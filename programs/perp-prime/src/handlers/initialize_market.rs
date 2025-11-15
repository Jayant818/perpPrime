
use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

use crate::{CircularQueue, EventQueue, EventQueueEntry, GlobalConfig, Market, QueueHeader, RequestItem, Slab};

#[derive(Accounts)]
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
            base_mint.key().as_ref(),
            quote_mint.key().as_ref(),
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

pub fn initialize_market(
    ctx:Context<InitializeMarket>,
    pair:String,
    oracle_price_feed:Pubkey,
    funding_rate:i64,
    funding_clamp:i64,
    initial_margin_rate:u64,
    maintainence_margin:u64,
    base_mint:Pubkey,
    quote_mint:Pubkey,
    base_lot_size:u64,
    quote_lot_size:u64,
)->Result<()>{

    let mut event_queue = ctx.accounts.event_queue.try_borrow_mut_data()?;
    let mut request_queue = ctx.accounts.request_queue.try_borrow_mut_data()?;

    let mut bids_slab = ctx.accounts.bids.try_borrow_mut_data()?;
    let mut asks_slab = ctx.accounts.asks.try_borrow_mut_data()?;
    

    CircularQueue::<RequestItem>::intialize(&mut request_queue, 128)?;

    CircularQueue::<EventQueueEntry>::intialize(&mut event_queue, 256)?;

    Slab::initialize(&mut bids_slab)?;
    Slab::initialize(&mut asks_slab)?;

    let market = &mut ctx.accounts.market;

    market.sequence = 0;
    market.asks = ctx.accounts.asks.key();
    market.bids = ctx.accounts.bids.key();
    market.asks_bump = ctx.bumps.asks;
    market.bids_bump = ctx.bumps.bids;
    market.request_queue = ctx.accounts.request_queue.key();
    market.event_queue = ctx.accounts.event_queue.key();
    market.oracle_price_feed = oracle_price_feed;
    market.pair = pair;
    market.funding_rate = funding_rate;
    market.funding_clamp = funding_clamp;
    market.last_funding_time = 0;
    market.cummulative_funding_rate = 0;
    market.base_mint = base_mint;
    market.quote_lot_size = quote_lot_size;
    market.base_lot_size = base_lot_size;
    market.quote_mint = quote_mint;
    market.maintainence_margin = maintainence_margin;
    market.initial_margin_rate = initial_margin_rate;
    market.event_queue_bump = ctx.bumps.event_queue;
    market.request_queue_bump = ctx.bumps.request_queue;
    market.market_bump = ctx.bumps.market;

    Ok(())
}