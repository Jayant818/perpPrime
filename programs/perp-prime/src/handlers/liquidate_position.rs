use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

use crate::{MARGIN_SCALE, Market, RequestItem, RequestQueueAccount, RequestType, UserAccount, UserPosition, error::ErrorCode, user};

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
            payer.key().as_ref(),
            market.key().as_ref()
        ],
        bump = user_position.bump,
    )]
    pub user_position: Account<'info,UserPosition>,

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
}

pub fn liquidate_position(ctx:Context<LiquidatePosition>)->Result<()>{

    let oracle_price:u64 = 1_000_000;


    let user_position = ctx.accounts.user_position;
    let market = ctx.accounts.market;
    
    require!(!user_position.is_liquidating, ErrorCode::PositionIsAlreadyLiquidating);

    let mut request_queue = ctx.accounts.request_queue.load_mut()?;

    // Computing Notional
    // using wider type for match to avoid overflow error
    let quantity = user_position.quantity as i128;
    let oracle_price_i128 = oracle_price as i128;

    let abs_qty = if qty >= 0 {quantity} else {-quantity};
    // We can take weighted average or somehow as we can't liquidate the user if the market is manipulated
    let notional  = abs_qty * oracle_price_i128; 

    let maintaince_margin  = notional.checked_mul(market.maintainence_margin as i128).ok_or(ErrorCode::MultiplicationError)?.checked_div(MARGIN_SCALE as i128).ok_or(ErrorCode::MathError)?;

    let avg_entry_price = user_position.avg_entry_price as i128;
    let pnl = (oracle_price_i128 - avg_entry_price) * quantity;

    // what if the unrealized_pnl is -ve
    let equity = (user_position.collateral as i128) + pnl;

    if equity < maintaince_margin{
        user_position.is_liquidating = true;

        // we have to reduce the size of the position
        let order_side = if user_position.quantity > 0 {
            // Long side, so we have to go to short  , that means I have to sell so side is ASK
            1
        }else {
            0
        };

        let quantity_to_lose = user_position.quantity.unsigned_abs();
        let user_key = ctx.accounts.user.key();
        let position_key = ctx.accounts.user_position.key();
        let order_id = 0;

        let requestItem = RequestItem::init(
            RequestType::LIQUIDATION, 
            1,//Limit Order 
            order_side, 
            position_key, 
            quantity_to_lose, 
            user_key, 
            order_id
        );

        request_queue.push(&requestItem)?;

    }


    Ok(())
}