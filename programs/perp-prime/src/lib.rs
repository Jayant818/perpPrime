pub mod constants;
pub mod error;
pub mod handlers;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use handlers::*;
pub use state::*;

declare_id!("Ec4ShFEJA2vRScZfsYtihn96vuiVYcGmN7xb1HqKAUke");

#[program]
pub mod perp_prime {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>,step_size:u8,fee_rate:u8,decimals:u8 ) -> Result<()> {
        handlers::initialize(ctx, step_size, fee_rate,decimals)?;

        Ok(())
    }


    pub fn initialize_market(
        ctx:Context<InitializeMarket>,
        pair:String,
        oracle_price_feed:Pubkey,
        funding_rate:i64,
        funding_clamp:i64,
        initial_margin_rate:u64,
        maintainence_margin:u64,
        base_mint:Pubkey,
        quote_mint:Pubkey,
        base_lot_size:u64,
        quote_lot_size:u64,
    )->Result<()>{

        handlers::initialize_market(ctx,pair,oracle_price_feed,funding_rate,funding_clamp,initial_margin_rate,maintainence_margin,base_mint,quote_mint,base_lot_size,quote_lot_size)?;

        Ok(())
    }

    pub fn deposit_collateral(ctx:Context<DepositCollateral>,amount:u64,decimals:u8)->Result<()>{
        
        handlers::deposit_collateral(ctx, amount, decimals)?;
        
        Ok(())
    }

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
        
        handlers::place_perp_order(ctx, amount_in_ui, side, qty_in_ui, _pair, position, margin, order_type, request_type)?;
        
        Ok(())
    }




}
