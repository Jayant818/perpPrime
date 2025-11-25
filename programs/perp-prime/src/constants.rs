use anchor_lang::prelude::*;

#[constant]
pub const SEED: &str = "anchor";

#[constant]
pub const PROGRAM_ID: &str = "Ec4ShFEJA2vRScZfsYtihn96vuiVYcGmN7xb1HqKAUke";

#[constant]
pub const PRICE_SCALE:i128 = 1_000_000_000; // for oracles

#[constant]
pub const FUNDING_SCALE:i128 = 1_000_000_000;

#[constant]
pub const MARGIN_SCALE:u8 = 100;
