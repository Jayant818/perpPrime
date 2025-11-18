use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use bytemuck::checked::from_bytes;

use crate::{CANCEL_EVENT, CancelEventPod, CircularQueue, EventQueue, EventQueueAccount, FILL_EVENT, FillEventPod, Market, error::ErrorCode};

#[derive(Accounts)]
pub struct ConsumeEvents<'info>{
    #[account(mut)]
    pub cranker: Signer<'info>,

    #[account(
        seeds = [
            b"market",
            base_mint.key().as_ref(),
            quote_mint.key().as_ref()
        ],
        bump,
    )]
    pub market: Account<'info,Market>, 

    /// CHECK:
    #[account(
        seeds = [
            b"event_queue",
            market.key().as_ref(),
        ],
        bump = market.event_queue_bump,
    )]
    pub event_queue: AccountLoader<'info,EventQueueAccount>,

    pub base_mint: InterfaceAccount<'info,Mint>,
    pub quote_mint:InterfaceAccount<'info,Mint>,
}

pub fn consume_event(ctx:Context<ConsumeEvents>)->Result<()>{

    let mut event_queue = ctx.accounts.event_queue.load_mut()?;
    let market = &ctx.accounts.market;

    let remaining_account_iter = &mut ctx.remaining_accounts.iter();

    let event = match event_queue.peek() ? {
        Some(event) => event,
        None => {
            msg!("Event Queue Empty");
            return Ok(())
        }
    };

    let raw_event_bytes = bytes_of(event.raw); 

    match event.raw.tag {
        FILL_EVENT =>{
            let fill_event:&FillEventPod = from_bytes(raw_event_bytes);

            let taker_pubkey = fill_event.taker;
            let maker_pubkey = fill_event.maker;

            msg!("Price: {}, Qty: {}", fill_event.price, fill_event.quantity);

        },
        CANCEL_EVENT =>{
            let cancel_event: &CancelEventPod = from_bytes(raw_event_bytes);

            let user = cancel_event.owner;
        },
        _=>{
            return Err(error!(ErrorCode::InvalidEventType))
        }
    }

    Ok(())
}