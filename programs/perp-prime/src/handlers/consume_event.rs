use anchor_lang::prelude::{pubkey::PubkeyError, *};
use anchor_spl::token_interface::Mint;
use bytemuck::{bytes_of, checked::{from_bytes, try_from_bytes}};
use crate::{OpenOrdersAccount, UserAccount, UserPosition, error::ErrorCode};

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

    #[account(
        mut,
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

pub fn consume_event(ctx:Context<ConsumeEvents>,max_events:u64)->Result<()>{

    let mut event_queue = ctx.accounts.event_queue.load_mut()?;
    let market = &ctx.accounts.market;

    for _ in 0.max_events {
        // first we are peeking 
        let event = match event_queue.peek()? {
            Some(item)=>item,
            None => break,
        };

        let raw_bytes = bytes_of(&event.raw);

        match event.raw.tag{
            FILL_EVENT =>{
                let fill_event:&FillEventPod = try_from_bytes(raw_bytes).map_err(|_| error!(ErrorCode::InvalidEventType))?;

                let maker_pk = Pubkey::new_from_array(fill_event.maker);
                let taker_pk = Pubkey::new_from_array(fill_event.taker);

                // Updating maker Position - Liquidity Provider
                process_position_update(
                    &ctx.remaining_accounts, 
                    &market.key(), 
                    maker_pk, 
                    fill_event.quantity, 
                    fill_event.price, 
                    fill_event.taker_side == 0  // if taker was bid , maker was Ask - shorting
                )?;

                process_position_update(
                    &ctx.remaining_accounts, 
                    &market.key(), 
                    taker_pk, 
                    fill_event.quantity, 
                    fill_event.price, 
                    fill_event.taker_side == 1, // if taker side is ask , then taker is selling and maker is buying,
                )?;

                msg!("FIlled: Price {} Qty {}", fill_event.price,fill_event.quantity);
            },
            CANCEL_EVENT =>{
                let cancel_event :&CancelEventPod = try_from_bytes(raw_bytes).map_err(|_| error!(ErrorCode::InvalidEventType))?;
                // probably have to remove from openOrdersAccount from here , but for that we need either client Id , or something 

                let user_pk = Pubkey::new_from_array(cancel_event.owner);

                // Determine user open_orders_account 
                let (expected_open_orders_account,_) = Pubkey::find_program_address(
                    &[
                        b"open_orders",
                        market.key().as_ref(),
                        user_pk.key().as_ref()
                    ], 
                    &crate::ID
                );

                let open_orders_account = match ctx.remaining_accounts.iter().find(|acc| acc.key() == expected_open_orders_account){
                    Some(data)=>data,
                    None => {
                        msg!("Warning: OpenOrders Account not provided for {}", owner_pk);
                        return Ok(())
                    }
                };

                let mut data = open_orders_account.try_borrow_mut_data()?;

                if data.len() < 8 {
                    return Err(error!(ErrorCode::MathError));
                }

                let mut slice = &data[8..];
                let mut open_orders_account:OpenOrdersAccount = AnchorDeserialize::deserialize(&mut slice)?;

                let order_idx = open_orders_account.find_order_by_order_id(cancel_event.order_id)?;

                let order = open_orders_account.orders[order_idx];

                order.status = crate::OrderStatus::FREE;
                // But what if the order is already partially filled than what we do in that case.
            },
            _=>{}
        }
    }

}

fn process_position_update(
    remaining_accounts:&[AccountInfo],
    market_key : &Pubkey,
    owner_pk : Pubkey,
    qty : u64,
    price:u64,
    is_short:bool, // true - selling
)->Result<()>{
    // Deriving the PDA for user position to make sure the we have provided the correct PDA
    let (expected_pda,_) = Pubkey::find_program_address
    (
        &[b"user_position",owner_pk.as_ref(),market_key.as_ref()], 
        &crate::ID,
    );

    // Searching for the remaining_accounts for this key
    let account_info = match remaining_accounts.iter().find(|acc| acc.key() == expected_pda){
        Some(acc)=>acc,
        None => {
            msg!("Warning: Position account not provided for {}", owner_pk);
            return Ok(())
        }
    };

    let mut data =  account_info.try_borrow_mut_data()?;

    if data.len() < 8 {
        return Err(error!(ErrorCode::MathError)?);
    }
    // skipping the 8-byte discriminator 
    let mut slice = &data[8..];
    let mut position:UserPosition = AnchorDeserialize::deserialize(&mut slice);

    let qty_i64 = qty as i64;
    let cost = (qty_i64.checked_mul(price as i64)).ok_or(ErrorCode::MathError)?;

    if is_short {
        // selling: Quantity decreases (become more negative)
        position.quantity = position.quantity.checked_sub(qty_i64).ok_or(ErrorCode::MathError)?;
        position.collateral = position.collateral.checked_sub(cost).ok_or(ErrorCode::MathError)?;
    }else {
        position.quantity = position.quantity.checked_add(qty_i64).ok_or(ErrorCode::MathError)?;
        position.collateral = position.collateral.checked_add(cost).ok_or(ErrorCode::MathError)?;
    }

    // serialize back the data 
    let mut writer = &mut data[8..];
    position.try_serialize(&mut writer)?;

    Ok(())
}