use anchor_lang::prelude::*;

use crate::{ Market, OpenOrdersAccount, Order, OrderStatus, RequestItem, RequestQueueAccount, RequestType, UserAccount, error::ErrorCode};

#[derive(Accounts)]
pub struct CancelOrder<'info>{
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [
            b"market",
            market.base_mint.key().as_ref(),
            market.quote_mint.key().as_ref(),
        ],
        bump = market.market_bump,
    )]
    pub market: Account<'info,Market>,

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
        seeds = [
            b"user_account",
            user.key().as_ref()
        ],
        bump = user_global_account.bump,
    )]
    pub user_global_account: Account<'info,UserAccount>,

    #[account(
        mut,
        seeds = [
            b"open_orders",
            owner.key().as_ref(),
            market.key().as_ref()
        ],
        bump = open_orders_account.bump,
    )]
    pub open_orders_account : Account<'info,OpenOrdersAccount>,
}

pub fn cancel_order(ctx:Context<CancelOrder>,client_order_id:u64)->Result<()>{

    let open_orders_account = &mut ctx.accounts.open_orders_account;

    // Finding the order by its Client ID - O(n)
    let order_index = open_orders_account.find_order_by_client_id(client_order_id)?;

    let order_to_cancel = & open_orders_account.orders[order_index];

    require!(order_to_cancel.status == OrderStatus::OPEN || order_to_cancel.status == OrderStatus::PENDING, ErrorCode::OrderAlreadyProcessed);

    let order_id = order_to_cancel.order_id;

    let request_item = RequestItem {
        order_id: order_id,
        order_side: order_to_cancel.side as u8,
        order_type:order_to_cancel.order_type as u8,
        position:order_to_cancel.position as u8,
        quantity:order_to_cancel.quantity,
        request_type: RequestType::CANCEL as u8,
        user: ctx.accounts.user.key(),
        padding0: [0;4],
    };

    let mut request_queue = ctx.accounts.request_queue.load_mut()?;
    request_queue.push(&request_item)?;

    Ok(())
}