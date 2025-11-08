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

    pub fn initialize(ctx: Context<Initialize>,step_size:u8,fee_rate:u8 ) -> Result<()> {
        handlers::initialize(ctx, step_size, fee_rate)?;

        Ok(())
    }


}
