
use anchor_lang::{ prelude::*};
use anchor_spl::token_interface::{TokenAccount, TokenInterface, TransferChecked, transfer_checked};

use crate::{CircularQueue, GlobalConfig, Market, Order, OrderPosition, OrderSide, OrderType, RequestItem, RequestType, UserAccount, UserPosition, error::ErrorCode, request_item, user_position};

#[derive(Accounts)]
#[instruction(_pair:String)]
pub struct PlacePerpOrder<'info>{

    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        seeds = [
            b"config",
        ],
        bump = config.config_bump,
    )]
    pub config: Account<'info,GlobalConfig>,

    #[account(
        mut,
        constraint = user_ata.mint == config.vault_mint @ErrorCode::VaultMintMismatch,
    )]
    pub user_ata : InterfaceAccount<'info,TokenAccount>,

    #[account(
        init_if_needed,
        payer = signer,
        space = UserAccount::INIT_SPACE + UserAccount::DISCRIMINATOR.len(),
        seeds = [
            b"user_account",
            signer.key().as_ref()
        ],
        bump,
    )]
    pub user_account : Account<'info,UserAccount>,

    #[account(
        mut,
        seeds = [
            b"market",
            _pair.as_bytes(),
        ],
        bump,
    )]
    pub market : Account<'info,Market>,

    #[account(
        init_if_needed,
        payer = signer,
        space = Order::INIT_SPACE + Order::DISCRIMINATOR.len(),
        seeds = [
            b"order",
            signer.key().as_ref(),
            market.key().as_ref(),
            market.sequence.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub order_pda : Account<'info,Order>,

    #[account(
        init_if_needed,
        payer = signer,
        space = UserPosition::INIT_SPACE + UserPosition::DISCRIMINATOR.len(),
        seeds = [
            b"user_position",
            market.key().as_ref(),
            signer.key().as_ref(),
        ],
        bump,
)]
    pub user_position : Account<'info,UserPosition>,

    #[account(
        mut,
        seeds = [
            b"vault"  
        ],
        bump = config.vault_bump,
        constraint = vault.key() == config.vault @ErrorCode::IncorrectVault,
        constraint = vault.mint == config.vault_mint @ErrorCode::VaultMintMismatch,
    )]
    pub vault: InterfaceAccount<'info,TokenAccount>,

    #[account(
        mut,
        seeds = [
            b"request_queue",
            market.base_mint.as_ref(),
            market.quote_mint.as_ref(),
        ],
        bump = market.request_queue_bump,
    )]
    // request queue account is raw byte buffer that contains a header + serialized items
    pub request_queue: UncheckedAccount<'info>,

    pub token_program: Interface<'info,TokenInterface>,

    pub system_program : Program<'info, System>,
}


// Place Perp Order 
// Motive : Main aim is to create a User Position account if it is not created yet, then user have to make a orders account also in that order value will be stored and currently it is in the pending state and then it will push that in the request queue, and we also have to publish an event so that cranker can listen to it.
pub fn place_perp_order(
    ctx:Context<PlacePerpOrder>, 
    amount_in_ui:u64, 
    side:OrderSide,
    qty_in_ui:u64,
    _pair:String,
    position:OrderPosition, 
    margin:u64,
    order_type:OrderType,
    request_type:RequestType
)->Result<()>{

    let user_account = &mut ctx.accounts.user_account;
    let market = &mut ctx.accounts.market;
    let order   = &mut ctx.accounts.order_pda;
    let vault = &mut ctx.accounts.vault;
    let user_position = &mut ctx.accounts.user_position;
    let config = &ctx.accounts.config;
    let user_ata = &ctx.accounts.user_ata;
    let user = &ctx.accounts.signer;
    let token_program: &Interface<'_, TokenInterface> = &ctx.accounts.token_program;
    let mut request_queue = ctx.accounts.request_queue.try_borrow_mut_data()?;

    // 1) Convert Amount in UI and quantity in UI into the Lots then store them,
    let quote_lot_size = market.quote_lot_size;
    let base_lot_size = market.base_lot_size;

    let amount_in_lots:u64 = amount_in_ui.checked_div(quote_lot_size).ok_or(ErrorCode::MathError)?;
    let size_in_lots:u64 = qty_in_ui.checked_div(base_lot_size).ok_or(ErrorCode::MathError)?;
    
    // suppose user is placing order in btc - usdc , either the user is providing the amount_in_ui in BTC , then we should get the current price of the BTC from oracle or perp price , then we get the amount of the USD, then we calculate the IMR for that market and check if user has passed that much amount of margin or not, we also check how much leverage user can get , I think it is 1/IMR
    
    // TODO: FETCH LATEST PRICE FROM THE ORACLE OR FETCH THE PERP PRICE.
    let latest_price:u64 = 10;

    let notional = latest_price.checked_mul(size_in_lots).ok_or(ErrorCode::MathError)?;
    let imr_for_this_market = market.initial_margin_rate;

    require!(margin >= notional.checked_mul(imr_for_this_market).ok_or(ErrorCode::MathError)?,ErrorCode::InsufficientMargin);

    // Check user has sufficient balance or not
    if user_account.collateral_balance < 0 {
        // fund the user account transfer the money.
        let amount_to_be_transferred = margin.checked_sub(user_account.collateral_balance).ok_or(ErrorCode::MathError)?;

        let cpi_acounts = TransferChecked{
            from:user_ata.to_account_info(),
            to:vault.to_account_info(),
            authority:user.to_account_info(),
            mint: token_program.to_account_info(),
        };

        let cpi_context = CpiContext::new(token_program.to_account_info(), cpi_acounts);

        transfer_checked(cpi_context, amount_to_be_transferred, config.decimals)?;      

        user_account.collateral_balance = user_account.collateral_balance.checked_add(amount_to_be_transferred).ok_or(ErrorCode::AdditionOverflow)?;  
    }

    market.sequence = market.sequence.checked_add(1).ok_or(ErrorCode::AdditionOverflow)?;

    // Fill the order PDA 
    order.side = side;
    order.bump = ctx.bumps.order_pda;
    order.position = position;
    order.quantity = qty_in_ui;
    order.entry_price = latest_price; // have to look at this also
    order.user = user.key();


    // Fill the User Position 
    user_position.market = market.key();
    user_position.bump = ctx.bumps.user_account;
    user_position.quantity = qty_in_ui;  // maybe we have to store the lot size here ?
    user_position.user_account = user_account.key();
    user_position.collateral = margin;
    

    // Now lock the balance in the User Account 
    // TODO: check if in user_account we have to store other things like bump and other things if we have initialized the user_account now ?
    user_account.collateral_balance  = user_account.collateral_balance.checked_sub(margin).ok_or(ErrorCode::InsufficientCollateral)?;
    user_account.locked_collateral = user_account.locked_collateral.checked_add(margin).ok_or(ErrorCode::AdditionOverflow)?; // can use checked_sum here 


    // we also have to push the position in the user_account, using user_account.positions, I don't think we have to it , if we have to do it tell why and how?
    let  pos_vec = &mut vec![user_position.key()];
    user_account.positions.append(pos_vec);


    // TODO: now we have to push the position into the Request Queue and emit the evenr
    let order_id:u128 = ((latest_price as u128 ) << 64 ) | (market.sequence as u128);
    let item = RequestItem {
        user: user.key(),
        order_id:order_id,
        order_side:side,
        order_type,
        position,
        quantity:qty_in_ui,
        request_type,
    };

    CircularQueue::<RequestItem>::push(&mut request_queue, &item)?;

    emit!(
        crate::events::OrderQueued{
            user: user.key(),
            market: market.key(),
            order_id,
            quantity: qty_in_ui,
            side,
            position
        }
    );

    Ok(())
}