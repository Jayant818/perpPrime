use anchor_lang::prelude::*;

use crate::{CircularQueue, Market, OpenOrdersAccount, Order, OrderStatus, RequestItem, RequestType, UserAccount};

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
            market.base_mint.as_ref(),
            market.quote_mint.as_ref(),
        ],
        bump = market.request_queue_bump,
    )]
    /// CHECK:
    pub request_queue: UncheckedAccount<'info>,

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

    let order_to_cancel = &mut open_orders_account.orders[order_index];

    let order_id = order_to_cancel.order_id;
    order_to_cancel.status = OrderStatus::CANCELLED;

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

    // clear the client_id from the lookup table
    open_orders_account.client_order_ids[order_index] = 0;

    let mut request_queue_data = ctx.accounts.request_queue.try_borrow_mut_data()?;
    CircularQueue::push(&mut request_queue_data, request_item)?;

    Ok(())
}