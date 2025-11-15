use anchor_lang::prelude::*;
use anchor_spl::token_interface::TokenAccount;
use bytemuck::bytes_of;

use crate::{CircularQueue, pubkey_to_array, array_to_pubkey, EventQueueEntry, FillEventPod, GlobalConfig, Market, OrderSide, RequestItem, Slab, SlabNode, error::ErrorCode};



fn emit_fill_event(
    event_queue: &mut [u8],
    maker: Pubkey,
    maker_order_id: u128,
    price: u64,
    quantity: u64,
    taker: Pubkey,
    taker_side: u8,
) -> Result<()> {
    let maker_bytes = pubkey_to_array(maker);
    let taker_bytes = pubkey_to_array(taker);

    let fill = FillEventPod {
        maker:maker_bytes,
        order_id: maker_order_id,
        price,
        quantity,
        taker:taker_bytes,
        taker_side: taker_side as u8,
        padding:[0;15],
    };


    let fill_bytes = bytes_of(&fill);
    let mut raw = [0u8;112];
    raw[..fill_bytes.len()].copy_from_slice(fill_bytes);

    let item = EventQueueEntry {
        raw,
        timestamp: Clock::get()?.unix_timestamp,
    };

    CircularQueue::<EventQueueEntry>::push(event_queue, &item)?;
    Ok(())
}

fn process_fill(
    bids: &mut [u8],
    event_queue: &mut [u8],
    taker_order: &mut RequestItem,
    max_bid_idx: u64,
    mut max_bid: SlabNode, 
) -> Result<()> {
    let filled_qty = if max_bid.quantity > taker_order.quantity {
        taker_order.quantity
    } else {
        max_bid.quantity
    };

    if max_bid.quantity > filled_qty {
        max_bid.quantity = max_bid.quantity.checked_sub(filled_qty).ok_or(ErrorCode::SubtractionUnderFlow)?;
        Slab::write_node(bids, max_bid_idx, &max_bid)?;
    } else {
        Slab::remove_by_key(bids, max_bid.key)?;
    }

    taker_order.quantity = taker_order.quantity.checked_sub(filled_qty).ok_or(ErrorCode::MathError)?;

    emit_fill_event(
        event_queue,
        array_to_pubkey(max_bid.owner),           
        max_bid.order_id,        
        Slab::get_price_from_key(max_bid.key),
        filled_qty,
        taker_order.user,
        taker_order.order_side,  
    )?;

    Ok(())
}


#[derive(Accounts)]
pub struct ProcessRequest<'info>{
    
    #[account(mut)]
    pub signer : Signer<'info>,

    #[account(
        seeds = [
           b"config" 
        ],
        bump = config.config_bump
    )]
    pub config : Account<'info,GlobalConfig>,

    #[account(
        constraint = base_mint.key() == config.vault_mint.key() @ErrorCode::MintMismatch,
    )]
    pub base_mint: InterfaceAccount<'info,TokenAccount>,

    pub quote_mint: InterfaceAccount<'info,TokenAccount>,

    #[account(
        seeds = [
            b"market",
            base_mint.key().as_ref(),
            quote_mint.key().as_ref()
        ],
        bump = market.market_bump,
    )]
    pub market : Account<'info,Market>,

    #[account(
        mut,
        seeds = [
            b"request_queue",
            base_mint.key().as_ref(),
            quote_mint.key().as_ref()
        ],
        bump = market.request_queue_bump,
    )]
    pub request_queue: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            b"event_queue",
            base_mint.key().as_ref(),
            quote_mint.key().as_ref()
        ],
        bump = market.event_queue_bump,
    )]
    pub event_queue: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            b"bids",
            base_mint.key().as_ref(),
            quote_mint.key().as_ref(),
        ],
        bump = market.bids_bump,
    )]
    pub bids: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            b"asks",
            base_mint.key().as_ref(),
            quote_mint.key().as_ref(),
        ],
        bump = market.asks_bump
    )]
    pub asks: UncheckedAccount<'info>,
}

const BID: u8 = 0;
const ASK: u8 = 1;
const MARKET_ORDER:u8 = 0;
const LIMIT_ORDER:u8 = 1;

pub fn process_request(ctx:Context<ProcessRequest>,pair:String)->Result<()>{

    let mut request_queue = ctx.accounts.request_queue.try_borrow_mut_data()?;
    let mut event_queue = ctx.accounts.event_queue.try_borrow_mut_data()?;

    let mut bids = ctx.accounts.bids.try_borrow_mut_data()?;
    let mut asks = ctx.accounts.asks.try_borrow_mut_data()?;

    let item = CircularQueue::<RequestItem>::pop(&mut request_queue)?;

    let mut taker_order = match item {
        Some(data)=>{
            data
        }
        None => {
            return Err(error!(ErrorCode::RequestQueueEmpty))
        }
    };

    // we also need to find the price of the order that we poped from the request_queue, we have to order id , so we need to specify the order PDA

    match taker_order.order_side{
        ASK =>{
                match taker_order.order_type {
                    MARKET_ORDER =>{
                        // Need to execute Immediately

                        if taker_order.quantity == 0 {
                            return Err(error!(ErrorCode::InvalidOrderQuantity))
                        }
                        // Loop will traverse until quantity become 0 or there are no item left in the bid orderbook
                        loop{
                            if taker_order.quantity == 0 {
                                break;
                            }

                            let bid_slab_header = Slab::read_header(&bids)?;

                            if bid_slab_header.order_count == 0 {
                                break;
                            }

                            let (max_bid_idx, max_bid) = Slab::find_max(&bids)?;
                            // compute price once
                            let max_bid_price = Slab::get_price_from_key(max_bid.key);

                            // process a single fill between taker_order and max_bid
                            process_fill(&mut bids, &mut event_queue, &mut taker_order, max_bid_idx, max_bid)?;

                            // if taker filled, we break
                            if taker_order.quantity == 0 {
                                break;
                            }

                            // otherwise loop to match remaining taker against next best bid
                        }

                        // Check if there is still some quantity left 
                        if taker_order.quantity > 0 {
                            // push that into the slab tree (policy: post leftover as resting ask)
                            Slab::insert(&mut asks, taker_order.order_id, taker_order.user, taker_order.quantity, taker_order.order_id)?;
                        }
                    },
                    LIMIT_ORDER =>{
                        // If It is limit order then we have to check if the price match or not , if the price doesn't match or some quantity is left then push the order in the Ask slab tree
                        let taker_order_price = get_price_from_order_id(taker_order.order_id);

                        loop{

                            if taker_order.quantity == 0 {
                                break;
                            }

                            let bid_slab_header = Slab::read_header(&bids)?;
                            if bid_slab_header.order_count == 0 {
                                // no bids left -> insert resting ask (leftover) and stop
                                Slab::insert(&mut asks, taker_order.order_id, taker_order.user, taker_order.quantity, taker_order.order_id)?;
                                break;
                            }

                            
                            let (max_bid_idx, max_bid) = Slab::find_max(&bids)?;
                            let max_bid_price = Slab::get_price_from_key(max_bid.key);

                            // buy price is less that the ask/sell price , so trade didn't happen , it will just sit in the ask orderbook
                            if max_bid_price < taker_order_price {
                                Slab::insert(&mut asks, taker_order.order_id, taker_order.user, taker_order.quantity, taker_order.order_id)?;
                                break;
                            }else {
                                // price crosses: execute a match
                                process_fill(&mut bids, &mut event_queue, &mut taker_order, max_bid_idx, max_bid)?;

                                // if taker filled, break
                                if taker_order.quantity == 0 {
                                    break;
                                }

                                // else continue (will match against next best bid)
                            }
                        }
                    },
                    _ => {
                        return Err(error!(ErrorCode::InvalidOrderType));
                    }
                }
        }
        BID =>{
            match taker_order.order_type{
                MARKET_ORDER=>{
                    // We are on the Buy side: match against asks (best ask = find_min)
                    if taker_order.quantity == 0 {
                        return Err(error!(ErrorCode::InvalidOrderQuantity))
                    }

                    loop {
                        if taker_order.quantity == 0 { break; }

                        let ask_slab_header = Slab::read_header(&asks)?;
                        if ask_slab_header.order_count == 0 {
                            break;
                        }

                        let (min_ask_idx, min_ask) = Slab::find_min(&asks)?;
                        // process a single fill between taker_order (buy) and min_ask (maker)
                        process_fill(&mut asks, &mut event_queue, &mut taker_order, min_ask_idx, min_ask)?;

                        if taker_order.quantity == 0 { break; }
                    }

                    if taker_order.quantity > 0 {
                        Slab::insert(&mut bids, taker_order.order_id, taker_order.user, taker_order.quantity, taker_order.order_id)?;
                    }
                },
                LIMIT_ORDER=>{
                    // Limit buy: only match if taker_price >= best ask price
                    let taker_order_price = get_price_from_order_id(taker_order.order_id);

                    loop {
                        if taker_order.quantity == 0 { break; }

                        let ask_slab_header = Slab::read_header(&asks)?;
                        if ask_slab_header.order_count == 0 {
                            Slab::insert(&mut bids, taker_order.order_id, taker_order.user, taker_order.quantity, taker_order.order_id)?;
                            break;
                        }

                        let (min_ask_idx, min_ask) = Slab::find_min(&asks)?;
                        let min_ask_price = Slab::get_price_from_key(min_ask.key);

                        if min_ask_price > taker_order_price {
                            // best ask too expensive -> post resting bid
                            Slab::insert(&mut bids, taker_order.order_id, taker_order.user, taker_order.quantity, taker_order.order_id)?;
                            break;
                        } else {
                            process_fill(&mut asks, &mut event_queue, &mut taker_order, min_ask_idx, min_ask)?;

                            if taker_order.quantity == 0 { break; }
                        }
                    }
                },
                _ => {
                    return Err(error!(ErrorCode::InvalidOrderType));
                }
            }
        },
        _ => {
            return Err(error!(ErrorCode::InvalidOrderSide));
        }
    }

    Ok(())
}


pub fn get_price_from_order_id(order_id:u128)->u64{
    (order_id >> 64 ) as u64
}
