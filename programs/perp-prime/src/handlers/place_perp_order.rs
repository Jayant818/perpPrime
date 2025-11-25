use anchor_lang::prelude::*;
use crate::{
    CircularQueue, GlobalConfig, Market, OpenOrdersAccount, Order, OrderPosition, OrderSide, OrderStatus, OrderType, RequestItem, UserAccount, error::ErrorCode, user
};

#[derive(Accounts)]
#[instruction(pair: String)]
pub struct PlacePerpOrder<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [b"config"],
        bump = config.config_bump,
    )]
    pub config: Account<'info, GlobalConfig>,

    // User's main "bank" account
    #[account(
        mut,
        seeds = [b"user_account", owner.key().as_ref()],
        bump = user_account.bump,
        has_one = owner @ ErrorCode::InvalidOwner,
    )]
    pub user_account: Account<'info, UserAccount>,

    #[account(
        mut,
        seeds = [b"market", pair.as_bytes()],
        bump = market.market_bump,
    )]
    pub market: Account<'info, Market>,

    #[account(
        mut,
        seeds = [
            b"open_orders",
            owner.key().as_ref(),
            market.key().as_ref()
        ],
        bump = open_orders_account.bump,
        has_one = owner @ ErrorCode::InvalidOwner,
        has_one = market,
    )]
    pub open_orders_account: Account<'info, OpenOrdersAccount>,
    
    #[account(
        mut,
        seeds = [
            b"request_queue",
            market.base_mint.as_ref(),
            market.quote_mint.as_ref(),
        ],
        bump = market.request_queue_bump,
    )]
    pub request_queue: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn place_perp_order(
    ctx: Context<PlacePerpOrder>,
    limit_price_in_lots: u64, 
    qty_in_lots: u64,
    side: OrderSide,
    position: OrderPosition,
    margin: u64,
    order_type: OrderType,
    client_order_id: u64,
) -> Result<()> {
    
    let user_account = &mut ctx.accounts.user_account;
    let market = &mut ctx.accounts.market;
    let open_orders_account = &mut ctx.accounts.open_orders_account;

    require!(user_account.available_collateral >= margin, ErrorCode::InsufficientCollateral);

    // we move margin from the available to locked, we don't touch the total_collateral
    user_account.available_collateral = user_account.available_collateral.checked_sub(margin).ok_or(ErrorCode::SubtractionUnderFlow)?;
    user_account.locked_collateral = user_account.locked_collateral.checked_add(margin).ok_or(ErrorCode::AdditionOverflow)?;

    let price_for_book = match order_type {
        OrderType::LimitOrder => limit_price_in_lots,
        OrderType::MarketOrder => {
            match side {
                // For a market buy, set price to MAX to match all available asks
                OrderSide::BID => u64::MAX,
                // For a market sell, set price to 0 to match all available bids
                OrderSide::ASK => 0, 
            }
        }
    };

    market.sequence = market.sequence.checked_add(1).ok_or(ErrorCode::AdditionOverflow)?;
    let order_id = (price_for_book as u128) << 64 | (market.sequence as u128);

    let free_slot_index = open_orders_account.find_free_slot()?;
    
    let order_slot = &mut open_orders_account.orders[free_slot_index];
    *order_slot = Order {
        status: OrderStatus::PENDING, 
        order_id,
        client_order_id,
        quantity: qty_in_lots,
        side,
        position,
        limit_price:limit_price_in_lots,
        order_type,
        locked_margin: margin,
        is_liquidating:false,
    };
    
    open_orders_account.client_order_ids[free_slot_index] = client_order_id;

    let request_item = RequestItem { 
        request_type: RequestType::OPEN,
        order_type : order_type as u8,
        order_side: side as u8,
        position : position as u8,
        padding0: [0;4],
        quantity: qty_in_lots,
        user: ctx.accounts.owner.key(),
        order_id,
    };

    let mut request_queue_data = ctx.accounts.request_queue.try_borrow_mut_data()?;
    CircularQueue::push(&mut request_queue_data, &request_item)?;

    Ok(())
}