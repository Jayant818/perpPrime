use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,

    #[msg("Incorrect Vault Mint Mismatch")]
    VaultMintMismatch,

    #[msg("Incorrect Mint Mismatch")]
    MintMismatch,

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

    #[msg("Request queue is Empty")]
    RequestQueueEmpty,

    #[msg("Invalid Order Quantity")]
    InvalidOrderQuantity,

    #[msg("Subtraction Underflow")]
    SubtractionUnderFlow,

    #[msg("Account Data too small")]
    AccountDataTooSmall,

    #[msg("Invalid Order Type")]
    InvalidOrderType,

    #[msg("Invalid Order Side")]
    InvalidOrderSide,

    #[msg("Max Order Reached")]
    MaxOrderReached,

    #[msg("Order not found")]
    OrderNotFound,

    #[msg("Invalid Owner Mismatch")]
    InvalidOwner,

    #[msg("order is not cancellable")]
    OrderNotCancelable,

    #[msg("Order Already Processed")]
    OrderAlreadyProcessed,

    #[msg("Order Id Mismatch")]
    OrderIdMismatch,

    #[msg("Invalid Event Type")]
    InvalidEventType,

    #[msg("Multiplication Error")]
    MultiplicationError,

    #[msg("Position is already Liquidating")]
    PositionIsAlreadyLiquidating,

    #[msg("Invalid Price Feed ID")]
    InvalidPriceFeedId,

    #[msg("Account is Locked")]
    AccountIsLocked,

    #[msg("Divison Underflow")]
    DivisonUnderFlow,

    #[msg("User Account Not Found")]
    UserAccountNotFound,
}   


