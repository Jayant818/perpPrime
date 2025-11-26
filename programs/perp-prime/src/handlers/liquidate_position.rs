use std::u64;

use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::{MARGIN_SCALE, Market, OpenOrdersAccount, Order, OrderPosition, OrderSide, PositionStatus, RequestItem, RequestQueueAccount, RequestType, UserAccount, UserPosition, error::ErrorCode, open_orders, user};

#[derive(Accounts)]
pub struct LiquidatePosition<'info>{
    #[account(mut)]
    pub liquidator : Signer<'info>,

    /// CHECK: 
    pub user: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            b"user_account",
            user.key().as_ref()
        ],
        bump = user_account.bump,
    )]
    pub user_account: Account<'info,UserAccount>,

    #[account(
        mut,
        seeds = [
            b"user_position",
            user.key().as_ref(),
            market.key().as_ref()
        ],
        bump = user_position.bump,
    )]
    pub user_position: Account<'info,UserPosition>,

    #[account(
        seeds = [
            b"open_orders",
            owner.key().as_ref(),
            market.key().as_ref()
        ],
        bump = open_orders_account.bump,
    )]
    pub open_order_account:Account<'info,OpenOrdersAccount>,

    #[account(
        seeds = [
            b"market",
            base_mint.key().as_ref(),
            quote_mint.key().as_ref(),
        ],
        bump = market.market_bump,
    )]
    pub market: Account<'info,Market>,

    #[account(
        mut,
        seeds  = [
            b"request_queue",
            market.key().as_ref(),
        ],
        bump,
    )]
    pub request_queue: AccountLoader<'info,RequestQueueAccount>,

    #[account(
        address = market.base_mint
    )]
    pub base_mint: InterfaceAccount<'info,Mint>,

    #[account(
        address = market.quote_mint
    )]
    pub quote_mint: InterfaceAccount<'info,Mint>,

    #[account(
        constraint = price_feed.key() == market.oracle_price_feed.key() @ErrorCode::InvalidPriceFeedId,
    )]
    pub price_feed: UncheckedAccount<'info>,
}

pub fn liquidate_position(ctx:Context<LiquidatePosition>)->Result<()>{

    let oracle_price:u64 = 1_000_000;


    let user_position = ctx.accounts.user_position;
    let market = ctx.accounts.market;
    
    require!(!user_position.is_liquidating, ErrorCode::PositionIsAlreadyLiquidating);

    let mut request_queue = ctx.accounts.request_queue.load_mut()?;
    let mut open_order_account = &mut ctx.accounts.open_order_account;

    // Computing Notional
    // using wider type for match to avoid overflow error
    let quantity_i128 = user_position.quantity as i128;
    let oracle_price_i128 = oracle_price as i128;

    let abs_qty = if qty >= 0 {quantity_i128} else {-quantity_i128};
    // We can take weighted average or somehow as we can't liquidate the user if the market is manipulated
    let notional_i128: i128  = abs_qty * oracle_price_i128; 

    let maintaince_margin_i128  = notional_i128.checked_mul(market.maintainence_margin as i128).ok_or(ErrorCode::MultiplicationError)?.checked_div(MARGIN_SCALE as i128).ok_or(ErrorCode::MathError)?;

    let entry_price_i128 = user_position.avg_entry_price as i128;
    let pnl_i128 = (oracle_price_i128 - entry_price_128) * quantity_i128;
    let collateral_i128 = user_position.collateral as i128;

    // what if the unrealized_pnl is -ve
    let equity_i128 = (collateral_i128 as i128).checked_add(pnl_i128).ok_or(ErrorCode::AdditionOverflow)?;

    if equity_i128 < maintaince_margin_i128{
        user_position.is_liquidating = true;

        let (order_side,position) = if quantity_i128 > 0 {
            (OrderSide::ASK, OrderPosition::SHORT)
        }else{
            (OrderSide::BID,OrderPosition::LONG)
        };

        // Limit_price - Price at which we can sell one quantity, it will differ for both Ask and Sell as we want to close the position, 
        // first calculate the collateral that user paid for single quantity 
        let price_for_single_quantity_i128 = collateral_i128.checked_div(quantity_i128.unsigned_abs()).unwrap_or(0);
        let limit_price_i128 = if order_side == OrderSide::ASK {
            entry_price_i128.checked_sub(price_for_single_quantity_i128).unwrap_or(0)
        }else {
            entry_price_i128.checked_add(price_for_single_quantity_i128).unwrap_or(u64::MAX as i128)
        };

        let limit_price = limit_price_i128.max(0) as u64;

        let quantity_to_lose = user_position.quantity.unsigned_abs();
        let user_key = ctx.accounts.user.key();
        let position_key = ctx.accounts.user_position.key();

        market.sequence = market.sequence.checked_add(1).ok_or(ErrorCode::AdditionOverflow)?;
        let order_id = (limit_price as u128) << 64 | market.sequence as u128;

        let free_slot = open_order_account.find_free_slot()?;

        let order = Order::new(
            crate::OrderStatus::PENDING, 
            quantity_to_lose, 
            order_id, 
            0,  // Liquidations Orders doesn't have order Id
            true, 
            order_side, 
            crate::OrderType::LimitOrder, 
            position,
            // I think in this case as perp order is placed so locked margin will be 0 
            0,  
            limit_price
        );

        open_order_account.orders[free_slot] = order;

        let request_item = RequestItem::init(
            RequestType::LIQUIDATION, 
            1,//Limit Order 
            order_side, 
            position_key, 
            quantity_to_lose, 
            user_key, 
            order_id
        );

        request_queue.push(&request_item)?;

        user_position.status = PositionStatus::Liquidating;

    }


    Ok(())
}