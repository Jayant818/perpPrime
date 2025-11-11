use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,

    #[msg("Incorrect Vault Mint Mismatch")]
    VaultMismatch,

    #[msg("Addition overflow occurred")]
    AdditionOverflow,

    #[msg("Queue is Full")]
    QueueIsFull
}
