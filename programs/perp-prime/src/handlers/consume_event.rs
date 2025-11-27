use anchor_lang::prelude::*;
use anchor_spl::{associated_token::spl_associated_token_account::solana_program::compute_units::sol_remaining_compute_units, token_interface::Mint};
use bytemuck::{bytes_of, checked::try_from_bytes};
use crate::{FUNDING_SCALE, OpenOrdersAccount, PositionStatus, UserAccount, UserPosition, error::ErrorCode};
use crate::{CANCEL_EVENT, CancelEventPod, EventQueueAccount, FILL_EVENT, FillEventPod, Market, OrderStatus};

#[derive(Accounts)]
pub struct ConsumeEvents<'info>{
    #[account(mut)]
    pub cranker: Signer<'info>,

    #[account(
        mut,
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

pub fn consume_event<'info>(ctx:Context<'_, '_, 'info, 'info, ConsumeEvents<'info>>,max_events:u64)->Result<()>{

    let event_queue = ctx.accounts.event_queue.load_mut()?;
    let market = &mut ctx.accounts.market;
    let clock = Clock::get()?;
    let current_ts = clock.unix_timestamp;

    // Updating global funding rate, event though no trades have happend , we still bring the global ts to now , so user pay fair.
    // also the oracle price may change so we don't want the new user to pay.
    let elapsed_time = current_ts.checked_sub(market.last_traded_ts).unwrap_or(0);
    if elapsed_time > 0 {
        let funding_added = market.current_funding_velocity.checked_mul(elapsed_time as i128).ok_or(ErrorCode::MultiplicationError)?;
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

                process_user_funding_settlement(&ctx.remaining_accounts, market, &market.key(), maker_pk)?;
                process_user_funding_settlement(&ctx.remaining_accounts, market, &market.key(), taker_pk)?;

                // Updating maker Position - Liquidity Provider
                process_position_update(
                    &ctx.remaining_accounts, 
                    &market.key(), 
                    maker_pk, 
                    fill_event.quantity, 
                    fill_event.price, 
                    // 0 - bid , 1- Ask
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

                // Convert [u64; 2] to u128
                let maker_order_id = (fill_event.maker_order_id[0] as u128) | ((fill_event.maker_order_id[1] as u128) << 64);
                let taker_order_id = (fill_event.taker_order_id[0] as u128) | ((fill_event.taker_order_id[1] as u128) << 64);

                // Reducing the locked margin & quantity from the OpenOrdersAccount
                process_order_fill(
                    &ctx.remaining_accounts, 
                    market.key(), 
                    maker_pk, 
                    maker_order_id, 
                    fill_event.quantity,
                )?; 

                // We have only one order Id stored so that's a problem we have to store the ID of other party also.
                process_order_fill(
                    &ctx.remaining_accounts, 
                    market.key(), 
                    taker_pk, 
                    taker_order_id, 
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
                        user_pk.as_ref()
                    ], 
                    &crate::ID
                );

                let open_orders_account_info = match ctx.remaining_accounts.iter().find(|acc| acc.key() == expected_open_orders_account){
                    Some(data)=>data,
                    None => {
                        msg!("Warning: OpenOrders Account not provided for {}", user_pk);
                        return Ok(())
                    }
                };

                // Determine user Account 
                let (expected_user_account,_) = Pubkey::find_program_address(
                    &[
                    b"user_account",
                    user_pk.as_ref()
                    ], 
                    &crate::ID
                );

                let user_account_info = match ctx.remaining_accounts.iter().find(|acc| acc.key() == expected_user_account){
                    Some(user)=> user,
                    None => {
                        msg!("Warning: User Account not provided for {}", user_pk);
                        return Ok(())
                    }
                };

                let open_orders_account_loader:AccountLoader<'_, OpenOrdersAccount> = AccountLoader::try_from(open_orders_account_info)?;
                let mut open_orders_account = open_orders_account_loader.load_mut()?;

                // Convert [u64; 2] to u128
                let order_id = (cancel_event.order_id[0] as u128) | ((cancel_event.order_id[1] as u128) << 64);
                let order_idx = open_orders_account.find_order_by_order_id(order_id)?;

                let locked_margin = open_orders_account.orders[order_idx].locked_margin;

                let order = &mut open_orders_account.orders[order_idx];

                order.set_status(OrderStatus::FILLED);
                order.order_id = [0u8; 16];
                order.locked_margin = 0;
                order.quantity = 0;
                open_orders_account.client_order_ids[order_idx] = 0;


                let mut user_account_data: std::cell::RefMut<'_, &mut [u8]> = user_account_info.try_borrow_mut_data()?;
                let mut user_account:UserAccount = AccountDeserialize::try_deserialize(&mut &user_account_data[..])?;

                user_account.locked_collateral = user_account.locked_collateral.checked_sub(locked_margin).ok_or(ErrorCode::SubtractionUnderFlow)?;
                user_account.available_collateral = user_account.available_collateral.checked_add(locked_margin).ok_or(ErrorCode::AdditionOverflow)?;

                // Serializing UserAccount Back
                let mut writer = &mut user_account_data.as_mut()[..];
                user_account.try_serialize(&mut writer)?;

                msg!("Refunded {} to {}", locked_margin, user_pk);
            },
            _=>{
                msg!("Unknown Event In the event queue found");
            }
        }
    }

    Ok(())
}

// releasing the margin as they are now moved to the userPosition PDA.
fn process_order_fill<'info>(
    remaining_accounts: &'info[AccountInfo<'info>],
    market_key:Pubkey,
    user_key:Pubkey,
    order_id:u128,
    filled_qty:u64,
)->Result<()>{

    let (open_order_pda,_) = Pubkey::find_program_address(&[
        b"open_orders",
        user_key.as_ref(),
        market_key.as_ref(),
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

    let open_orders_account_loader: AccountLoader<'_, OpenOrdersAccount> = AccountLoader::try_from(open_order_account_info)?;

    let mut open_orders_account = open_orders_account_loader.load_mut()?; 

    let order_idx = open_orders_account.find_order_by_order_id(order_id)?;
    let order = &mut open_orders_account.orders[order_idx];

    order.quantity = order.quantity.checked_sub(filled_qty).ok_or(ErrorCode::SubtractionUnderFlow)?;
    // This is a proportion of the margin that we use

    // We convert the data to u128 while doing operation so that overflow didn't happen.
    let margin_release = (filled_qty as u128)
                                .checked_mul(order.locked_margin as u128)
                                .ok_or(ErrorCode::MathError)?
                                .checked_div(order.quantity as u128)
                                .ok_or(ErrorCode::MathError)? as u64;

    order.locked_margin = order.locked_margin.checked_sub(margin_release).ok_or(ErrorCode::SubtractionUnderFlow)?;

    if order.quantity == 0 {
        order.set_status(OrderStatus::FREE);
        open_orders_account.client_order_ids[order_idx] = 0;
    }

    Ok(())
}

// reference is also tied to 'info
// the reference and the inner AccountInfo should share the same reference.
// Accountloader stores a reference to AccountInfo<'info>, so it must not outlive AccountInfo<'info> so we are passing the specifier in the lifetime also.
// fn load_open_order_from_info<'info>(acc:&'info AccountInfo<'info>)->Result<AccountLoader<'info,OpenOrdersAccount>>{
//     AccountLoader::try_from(acc)
// }

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
        return Err(ErrorCode::MathError.into());
    }
    let mut position:UserPosition = AccountDeserialize::try_deserialize(&mut &data[..])?;

    let qty_i64 = qty as i64;
    let cost = qty.checked_mul(price).ok_or(ErrorCode::MathError)?;

    if is_short {
        // selling: Quantity decreases (become more negative)
        position.quantity = position.quantity.checked_sub(qty_i64).ok_or(ErrorCode::MathError)?;
        position.collateral = position.collateral.checked_sub(cost).ok_or(ErrorCode::MathError)?;
    }else {
        position.quantity = position.quantity.checked_add(qty_i64).ok_or(ErrorCode::MathError)?;
        position.collateral = position.collateral.checked_add(cost).ok_or(ErrorCode::MathError)?;
    }

    // Updating the average_entry_price if we are increasing the position size , but if we are closing a position then average entry price remains same
    // Increasing - Weighted Average
    // Decreasing - No Change
    // Flipping - Reset to Trade Price 
    let is_increasing = (position.quantity > 0 && !is_short)  || (position.quantity < 0  && is_short) || position.quantity == 0;

    if is_increasing {
        // avg - total value / total qty
        let old_val = (position.quantity as i128).checked_mul(position.avg_entry_price as i128).ok_or(ErrorCode::MultiplicationError)?;
        let new_val = (qty as i128).checked_mul(price as i128).ok_or(ErrorCode::MultiplicationError)?;

        let total_qty = (position.quantity as i128).checked_add(qty as i128).ok_or(ErrorCode::AdditionOverflow)?;

        let new_avg_val = old_val.checked_add(new_val).ok_or(ErrorCode::AdditionOverflow)?.checked_div(total_qty).ok_or(ErrorCode::DivisonUnderFlow)?;

        if qty > 0 {
            position.avg_entry_price = new_avg_val as u64;
        }
    }else {
        // we are decreasing or flipping 
        let remaining_qty_signed = if is_short {
            // shorting , means on the Ask Slab 
            // as we are decreasing the position so qty will be in +ve , so as we have to decrease the position so we will subtract 
            (position.quantity as i128).checked_sub(qty as i128).ok_or(ErrorCode::SubtractionUnderFlow).unwrap()
        }else{
            // Long , means on the Bid Slab
            // as we are decreasing the position so qty will be in -ve , to change the position we need to add
            (position.quantity as i128).checked_add(qty as i128).ok_or(ErrorCode::AdditionOverflow).unwrap()
        };

        // if we are flipping 
        // from long to short then qty will go from +ve to -ve and remaining_qty should be -ve, so qty is +ve & rem_qty is -ve 
        // from short to long then qty will go from -ve to +ve and remaining_qty should be +ve, so qty is -ve and rem_qty is +ve 
        let is_flipping =  position.quantity > 0 && remaining_qty_signed < 0  ||
            position.quantity < 0 && remaining_qty_signed > 0;

        if is_flipping{
            // we get a new trade price 
            position.avg_entry_price = price;
        }
    }

    if position.quantity == 0{
        position.status = PositionStatus::Active;
        position.avg_entry_price = 0;
    }

    // serialize back the data 
    let mut writer = &mut data[8..];
    position.try_serialize(&mut writer)?;

    Ok(())
}

pub fn process_user_funding_settlement(
    remaining_accounts: &[AccountInfo],
    market: &Market,
    market_key: &Pubkey,
    user_pk: Pubkey
)->Result<()>{

    let (expected_pda,_) = Pubkey::find_program_address(&[
        b"user_position",
        user_pk.as_ref(),
        market_key.as_ref(),
    ], &crate::ID);

    let user_account= match remaining_accounts.iter().find(|acc| acc.key() == expected_pda){
        Some(user_account)=>user_account,
        None=> return Err(ErrorCode::UserAccountNotFound.into()),
    };
    
    let mut user_account_data = user_account.try_borrow_mut_data()?;

    let mut user_position : UserPosition = AccountDeserialize::try_deserialize(&mut &user_account_data[..])?;

    let user_cum = user_position.last_cumulative_funding_rate;
    let global_cummulative_funding_rate = market.cummulative_funding_rate;

    let diff = global_cummulative_funding_rate - user_cum;

    // if Qty > 0 (Long) and diff > 0 (+ve) : User Pays
    // If Qty < 0 (Short) and diff > 0 (+ve) : User receives
    // getting payment = Position_size * diff
    // for the new user the funding rate will be 0, as the quantity is 0 
    let funding_payment_scaled = diff.checked_mul(user_position.quantity as i128).ok_or(ErrorCode::MultiplicationError)?;
    let funding_payment = funding_payment_scaled / FUNDING_SCALE;

    // +ve means user pays, -ve means user receives
    if funding_payment > 0 {
        let payment = funding_payment as u64;
        user_position.collateral = user_position.collateral.checked_sub(payment).ok_or(ErrorCode::InsufficientCollateral)?;
    } else if funding_payment < 0 {
        let payment = funding_payment.abs() as u64;
        user_position.collateral = user_position.collateral.checked_add(payment).ok_or(ErrorCode::AdditionOverflow)?;
    }

    user_position.last_cumulative_funding_rate = global_cummulative_funding_rate;

    let mut writer = &mut user_account_data.as_mut()[..];
    user_position.try_serialize(&mut writer)?;

    Ok(())
}