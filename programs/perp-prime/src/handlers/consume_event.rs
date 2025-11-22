use anchor_lang::prelude::*;
use anchor_spl::{associated_token::spl_associated_token_account::solana_program::compute_units::sol_remaining_compute_units, token_interface::Mint};
use bytemuck::{bytes_of, checked::{from_bytes, try_from_bytes}};
use crate::{FUNDING_SCALE, OpenOrdersAccount, UserAccount, UserPosition, error::ErrorCode, user, user_position};

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
    let clock = Clock::get()?;
    let current_ts = clock.unix_timestamp;

    // Updating global funding rate, event though no trades have happend , we still bring the global ts to now , so user pay fair.
    // also the oracle price may change so we don't want the new user to pay.
    let elapsed_time = current_ts.checked_sub(current_ts).ok_or(ErrorCode::SubtractionUnderFlow)?;
    if elapsed_time > 0 {
        let funding_added = market.current_funding_velocity.checked_mul(elapsed_time).ok_or(ErrorCode::MultiplicationError)?;
        market.cummulative_funding_rate = market.cummulative_funding_rate.checked_add(funding_added).ok_or(ErrorCode::AdditionOverflow)?;
        market.last_traded_ts = current_ts;
    }

    let limit = std::cmp::min(event_queue.header.count, max_events);

    for _ in 0..limit {

        // If we are low on CU, then stop
        if sol_remaining_compute_units() < 5000 {
            msg!("Compute budget low, exiting event loop early.");
            break;
        }

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

                process_user_funding_settlement(&ctx.remaining_accounts, &market, maker_pk)?;
                process_user_funding_settlement(&ctx.remaining_accounts, &market, taker_pk)?;

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

                // Reducing the locked margin & quantity from the OpenOrdersAccount
                process_order_fill(
                    &ctx.remaining_accounts, 
                    &market.key(), 
                    maker_pk, 
                    fill_event.maker_order_id, 
                    fill_event.quantity,
                )?; 

                // We have only one order Id stored so that's a problem we have to store the ID of other party also.
                process_order_fill(
                    &ctx.remaining_accounts, 
                    &market.key(), 
                taker_pk, 
                    fill_event.taker_order_id, 
                    fill_event.quantity
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

                // Determine user Account 
                let (expected_user_account,_) = Pubkey::find_program_address(
                    &[
                    b"user_account",
                    user_pk.key().as_ref()
                    ], 
                    &crate::ID
                );

                let user_account_info = match ctx.remaining_accounts.iter().find(|acc| acc.key() == expected_user_account){
                    Some(user)=> user,
                    None => {
                        msg!("Warning: User Account not provided for {}", owner_pk);
                        return Ok(())
                    }
                };

                let mut data = open_orders_account.try_borrow_mut_data()?;

                let mut open_orders_account:OpenOrdersAccount = AnchorDeserialize::deserialize(&mut data)?;

                let order_idx = open_orders_account.find_order_by_order_id(cancel_event.order_id)?;

                let order = &mut open_orders_account.orders[order_idx];

                order.status = crate::OrderStatus::FREE;
                order.order_id = 0;
                order.locked_margin = 0;
                order.quantity = 0;
                open_orders_account.client_order_ids[order_idx] = 0;

                let mut writer = &mut data[..];
                open_orders_account.try_serialize(&mut writer);

                let user_account_data = user_account_info.try_borrow_mut_data()?;
                let user_account:UserAccount = AccountDeserialize::try_deserialize(&mut user_account_data.as_ref())?;

                user_account.locked_collateral = user_account.locked_collateral.checked_sub(order.locked_margin).ok_or(ErrorCode::SubtractionUnderFlow)?;
                user_account.available_collateral = user_account.available_collateral.checked_add(order.locked_margin).ok_or(ErrorCode::AdditionOverflow)?;

                // Serializing UserAccount Back
                let mut writer = &mut user_account_data[..];
                user_account.try_serialize(&mut writer)?;

                msg!("Refunded {} to {}", locked_margin, user_key);
                Ok(())

            },
            _=>{
                msg!("Unknown Event In the event queue found");
                Ok(())
            }
        }
    }

}

// releasing the margin as they are now moved to the userPosition PDA.
fn process_order_fill(
    remaining_accounts: &[AccountInfo],
    market_key:Pubkey,
    user_key:Pubkey,
    order_id:u128,
    filled_qty:u64,
)->Result<()>{

    let (open_order_pda,_) = Pubkey::find_program_address(&[
        b"open_orders",
        user_key.key().as_ref(),
        market_key.key().as_ref(),
    ], &crate::ID);

    let open_order_account_info = match remaining_accounts.iter().find(|acc| acc.key() == open_order_pda) {
        Some(data) => {
            data
        }
        None=> {
            msg!("Warning: Open Order Account is not provided for {}",user_key);
            return Ok(())
        }
    };

    let mut data = open_order_account_info.try_borrow_mut_data()?;

    let mut open_orders_account:OpenOrdersAccount = AccountDeserialize::deserialize(&mut &data)?;

    let order_idx = open_orders_account.find_order_by_order_id(order_id)?;
    let order = open_orders_account.orders[order_idx];

    order.quantity = order.quantity.checked_sub(filled_qty).ok_or(ErrorCode::SubtractionUnderFlow)?;
    // This is a proportion of the margin that we use

    // We convert the data to u128 while doing operation so that overflow didn't happen.
    let margin_release = (filled_qty as u128).checked_mul(order.locked_margin as u128).ok_or(ErrorCode::MathError)?.checked_div(order.quantity as u128).ok_or(ErrorCode::MathError)? as u64;
    order.locked_margin = order.locked_margin.checked_sub(ErrorCode::SubtractionUnderFlow)?;

    if order.quantity == 0 {
        order.status = crate::OrderStatus::FREE;
        open_orders_account.client_order_id[order_idx] = 0;
    }

    open_orders_account.try_serialize(&mut order)?;
    Ok(())
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
    let mut position:UserPosition = AccountDeserialize::try_deserialize(&mut &data)?;

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

pub fn process_user_funding_settlement(
    remaining_accounts: &[AccountInfo],
    market: &Account<'info,Market>,
    user_pk: Pubkey
)->Result<()>{

    let (expected_pda,_) = Pubkey::find_program_address(&[
        b"user_position",
        user_pk.as_ref(),
        market.key().as_ref(),
    ], &crate::ID);

    let user_account= remaining_accounts.iter().find(|acc| acc.key() == expected_pda)?;
    
    let user_account_data = user_account.try_borrow_mut_data()?;

    let user_position : UserPosition = AccountDeserialize::try_deserialize(&mut &user_account_data)?;

    let user_cum = user_position.last_cumulative_funding_rate;
    let global_cummulative_funding_rate = market.cummulative_funding_rate;

    let diff = global_cummulative_funding_rate - user_cum;

    // if Qty > 0 (Long) and diff > 0 (+ve) : User Pays
    // If Qty < 0 (Short) and diff > 0 (+ve) : User receives
    // getting payment = Position_size * diff
    let funding_payment_scaled = diff.checked_mul(user_position.quantity).ok_or(ErrorCode::MultiplicationError)?;
    let funding_payment = funding_payment_scaled / FUNDING_SCALE;

    // +ve
    if funding_payment > 0 {
        user_position.collateral = user_position.collateral.checked_sub(funding_payment).ok_or(ErrorCode::InsufficientCollateral)?;
    }else{
        user_position.collateral = user_position.collateral.checked_add(funding_payment).ok_or(ErrorCode::AdditionOverflow)?;
    }

    user_position.last_cumulative_funding_rate = user_cum;

    user_position.try_serialize(&mut user_account_data)?;

    Ok(())
}