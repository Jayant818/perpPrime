use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,

    #[msg("Incorrect Vault Mint Mismatch")]
    VaultMintMismatch,

    #[msg("Addition overflow occurred")]
    AdditionOverflow,

    #[msg("Math Error")]
    MathError,

    #[msg("Queue is Full")]
    QueueIsFull,

    #[msg("Insufficient Margin")]
    InsufficientMargin,

    #[msg("Incorrect Vault")]
    IncorrectVault,

    #[msg("Insufficient Collateral")]
    InsufficientCollateral,

    #[msg("Index is out of bound")]
    IndexOutOfBound,

    #[msg("Unexpected Node Tag")]
    UnexpectedNodeTag,

    #[msg("Duplicate Key")]
    DuplicateKey,

    #[msg("Empty Slab")]
    EmptySlab,

    #[msg("Key didn't match, Index Out of Bound")]
    KeyMisMatch,
}   

