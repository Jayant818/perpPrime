use anchor_lang::prelude::*;
use crate::{
    CircularQueue, GlobalConfig, Market, Order, OrderPosition, OrderSide,
    OrderStatus, OrderType, RequestItem, 
    UserAccount, OpenOrdersAccount, error::ErrorCode
};

#[derive(Accounts)]
#[instruction(pair: String)]
pub struct PlacePerpOrder<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        seeds = [b"config"],
        bump = config.config_bump,
    )]
    pub config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [b"user_account", signer.key().as_ref()],
        bump = user_account.bump, 
        has_one = owner @ ErrorCode::InvalidOwner,
    )]
    pub user_account: Account<'info, UserAccount>,

    #[account(
        mut,
        seeds = [b"market", pair.as_bytes()],
        bump,
    )]
    pub market: Account<'info, Market>,

    #[account(
        mut,
        seeds = [
            b"open_orders",
            signer.key().as_ref(),
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
    price_in_lots: u64, // Price must be in lots
    qty_in_lots: u64,   // Qty must be in lots
    side: OrderSide,
    position: OrderPosition,
    margin: u64,
    order_type: OrderType,
    client_order_id: u64,
) -> Result<()> {
    
    let user_account = &mut ctx.accounts.user_account;
    let market = &mut ctx.accounts.market;
    let open_orders_account = &mut ctx.accounts.open_orders_account;

    require!(user_account.collateral_balance >= margin, ErrorCode::InsufficientMargin);

    // Lock margin 
    user_account.collateral_balance = user_account.collateral_balance.checked_sub(margin).ok_or(ErrorCode::MathError)?;
    user_account.locked_collateral = user_account.locked_collateral.checked_add(margin).ok_or(ErrorCode::MathError)?;

    market.sequence = market.sequence.checked_add(1).ok_or(ErrorCode::AdditionOverflow)?;
    let order_id = (price_in_lots as u128) << 64 | (market.sequence as u128);

    // Find and fill an order slot in the OpenOrdersAccount
    let free_slot_index = open_orders_account.find_free_slot()?;
    
    let order_slot = &mut open_orders_account.orders[free_slot_index];
    *order_slot = Order {
        status: OrderStatus::PENDING,
        order_id,
        client_order_id,
        quantity: qty_in_lots,
        side,
        position,
        locked_margin: margin,
        entry_price: price_in_lots, 
    };
    
    // Fill the parallel lookup array
    open_orders_account.client_order_ids[free_slot_index] = client_order_id;

    // Push to the Request Queue
    let request_item = RequestItem { 
        request_type: RequestType::OPEN,
        order_type:order_type as u8,
        order_side: side as u8,
        position:position as u8,
        padding0:[0;4],
        quantity: qty_in_lots,
        user: ctx.accounts.signer.key(),
        order_id,
    };

    let mut request_queue_data = ctx.accounts.request_queue.try_borrow_mut_data()?;
    CircularQueue::push(&mut request_queue_data, &request_item)?;

    Ok(())
}