use anchor_lang::prelude::*;
use anchor_spl::token_interface::TokenAccount;
use bytemuck::{bytes_of, checked::cast};

use crate::{AnyEvent, CancelEventPod, CircularQueue, EventQueueAccount, EventQueueEntry, FillEventPod, GlobalConfig, Market, OpenOrdersAccount, OrderSide, OrderStatus, RequestItem, RequestQueueAccount, Slab, SlabNode, array_to_pubkey, error::ErrorCode, pubkey_to_array};

fn emit_fill_event(
    event_queue: &mut std::cell::RefMut<'_, EventQueueAccount>,
    maker: Pubkey,
    maker_order_id: u128,
    price: u64,
    quantity: u64,
    taker: Pubkey,
    taker_side: u8,
) -> Result<()> {
    let maker_bytes = pubkey_to_array(maker);
    let taker_bytes = pubkey_to_array(taker);

    let fill = FillEventPod::new(
        pubkey_to_array(maker),
        maker_order_id,
        price,
        quantity,
        pubkey_to_array(taker),
        taker_side
    );

    // let fill_bytes = bytes_of(&fill);
    // let mut raw = [0u8;112];
    // raw[..fill_bytes.len()].copy_from_slice(fill_bytes);

    let raw_event:AnyEvent = cast(fill);

    let item = EventQueueEntry {
        raw:raw_event,
        timestamp: Clock::get()?.unix_timestamp,
    };

    event_queue.push(&item);
    Ok(())
}


fn process_fill(
    bids: &mut [u8],
    event_queue: &mut std::cell::RefMut<'_, EventQueueAccount>,
    taker_order: &mut RequestItem,
    max_bid_idx: u64,
    mut max_bid: SlabNode, 
    market: &mut Account<'info,Market>,
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

    let price = get_price_from_order_id(max_bid.order_id);
    market.last_price = price;
    market.last_funding_time = Clock::get()?.unix_timestamp;

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
        mut,
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
            market.key().as_ref(),
        ],
        bump = market.request_queue_bump,
    )]
    pub request_queue: AccountLoader<'info,RequestQueueAccount>,

    #[account(
        mut,
        seeds = [
            b"event_queue",
            market.key().as_ref(),
        ],
        bump = market.event_queue_bump,
    )]
    pub event_queue: AccountLoader<'info,EventQueueAccount>,

    #[account(
        mut,
        seeds = [
            b"bids",
            market.key().as_ref(),
        ],
        bump = market.bids_bump,
    )]
    pub bids: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            b"asks",
            market.key().as_ref(),
        ],
        bump = market.asks_bump
    )]
    pub asks: UncheckedAccount<'info>,

    // This is provided by cranker by peeking the request Queue. Also we don't need the userAccount, cuz lock phle kar diya hai and unlocking and settling wagara consume_event mai hoga 
    #[account(
        // should cranker provide the user also
        // seeds = [
        //     b"open_orders",
        //     owner.key().as_ref(),
        //     market.key().as_ref()
        // ],
        // bump = open_orders_account.bump,
    )]
    pub open_orders_account: Account<'info,OpenOrdersAccount>,
}

const BID: u8 = 0;
const ASK: u8 = 1;
const MARKET_ORDER:u8 = 0;
const LIMIT_ORDER:u8 = 1;
const OPEN : u8 = 0;
const CANCEL : u8 = 0;

pub fn process_request(ctx:Context<ProcessRequest>,pair:String)->Result<()>{

    let mut request_queue = ctx.accounts.request_queue.load_mut()?;
    let mut event_queue: std::cell::RefMut<'_, EventQueueAccount> = ctx.accounts.event_queue.load_mut()?;

    let mut bids = ctx.accounts.bids.try_borrow_mut_data()?;
    let mut asks = ctx.accounts.asks.try_borrow_mut_data()?;

    let open_orders_account = &mut ctx.accounts.open_orders_account;

    let peeked_value = CircularQueue::<RequestItem>::peek(&mut request_queue)?;

    let market = &mut ctx.accounts.market;

    let request_item = match peeked_value {
        Some(item)=>item,
        None => return Ok(())
    };

    // Check : cranker provided open order account owner should match request Item Owner
    require_keys_eq!(taker_order.user, open_orders_account.owner, ErrorCode::InvalidOwner);

    match request_item.request_type {
        OPEN => {
            handle_place_order(&request_item, open_orders_account, &mut *bids, &mut *asks, &mut event_queue,&mut market)?;
        },
        CANCEL =>{
            handle_cancel_order(&request_item, open_orders_account, &mut *bids, &mut *asks, &mut event_queue)?;
        }
    }

    request_queue.pop()?;


    // we also need to find the price of the order that we poped from the request_queue, we have to order id , so we need to specify the order PDA

     Ok(())
}


// TODO: Market order are IOC, so it doesn't actually mean to store them in the tree, reject the order for the remaining quantity.
fn handle_place_order(
    request_item:&RequestItem,
    open_orders_account: &mut Account<OpenOrdersAccount>,
    bids: &mut [u8],
    asks: &mut [u8],
    event_queue: &mut std::cell::RefMut<'_, EventQueueAccount>,
    market: &mut Account<'info,Market>,
)->Result<()>{

    let order_idx = open_orders_account.find_order_by_order_id(order.order_id)?;
    let order = &mut open_orders_account.orders[order_idx];

    require!(order.status == OrderStatus::PENDING, ErrorCode::OrderAlreadyProcessed);
    require_eq!(order.order_id,request_item.order_id,ErrorCode::OrderIdMismatch);

    order.status = OrderStatus::OPEN;

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
                            process_fill(&mut bids, &mut event_queue, &mut taker_order, max_bid_idx, max_bid,market)?;

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
                                process_fill(&mut bids, &mut event_queue, &mut taker_order, max_bid_idx, max_bid,market)?;

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
                        process_fill(&mut asks, &mut event_queue, &mut taker_order, min_ask_idx, min_ask,market)?;

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
                            process_fill(&mut asks, &mut event_queue, &mut taker_order, min_ask_idx, min_ask,market)?;

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

fn handle_cancel_order(
    request:&RequestItem,
    open_orders_account: &mut Account<OpenOrdersAccount>,
    bids: &mut [u8],
    asks: &mut [u8],
    event_queue: &mut std::cell::RefMut<'_, EventQueueAccount>,
)->Result<()>{

    let order_index = open_orders_account.find_order_by_order_id(request.order_id)?;
    let order = &mut open_orders_account.orders[order_index];

    // what about PENDING order here , if the order is pending means it is in the request queue and will be processed before the cancel order , so we never encounter the PENDING state
    if order.status == OrderStatus::FREE || order.status == OrderStatus::FILLED {
        return Ok(())
    }

    let node:SlabNode;

    let remove_result = match order.side {
        OrderSide::BID =>{
            // We are on the bid side 
            Slab::remove_by_key( bids, request.order_id)
        },
        OrderSide::ASK=>{
            Slab::remove_by_key(asks, request.order_id)
        }
    };

    // For the case where order is OPEN, but is filled completely so it is not in the slab Tree.
    match remove_result{
        Ok(node)=>{
            let cancel_event = CancelEventPod::new(
                node.order_id, 
                node.owner, 
                node.quantity
            );

            let raw_event = cast(cancel_event);

            let item = EventQueueEntry{
                raw:raw_event,
                timestamp:Clock::get()?.unix_timestamp,
            };

            event_queue.push(&item);

            order.status = OrderStatus::CANCELLED;

        },
        Err=>{
            msg!("Order not found in the slab, assuming either it is filled or already cancelled ");
        }
    }

    Ok(())
}


pub fn get_price_from_order_id(order_id:u128)->u64{
    (order_id >> 64 ) as u64
}
