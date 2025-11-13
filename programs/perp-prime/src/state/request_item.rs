use anchor_lang::prelude::{borsh::de, *};

#[derive(Clone,Copy,AnchorDeserialize,AnchorSerialize,Default)]
#[repr(C)]
pub enum OrderType{
    #[default]
    MarketOrder,
    LimitOrder
}

#[repr(C)]
#[derive(Clone,Copy,Debug,AnchorDeserialize,AnchorSerialize,Default,InitSpace)]
pub enum OrderSide{
    #[default]
    BID = 0,
    ASK = 1
}

#[repr(C)]
#[derive(Clone,AnchorDeserialize,AnchorSerialize,InitSpace,Default)]
pub enum OrderPosition{
    #[default]
    SHORT = 0,
    LONG = 1
}

#[repr(C)]
#[derive(Clone,Copy,AnchorDeserialize,AnchorSerialize,Default)]
pub enum RequestType{
    #[default]
    OPEN,
    CANCEL
}

#[repr(C)]
#[derive(Default)]
#[account]
pub struct RequestItem{
    pub request_type:RequestType,
    pub order_type: OrderType,
    pub order_side :OrderSide,
    pub position: OrderPosition,
    pub quantity: u64,
    pub user: Pubkey,
    pub order_id: u128,
    // pub entry_price: u64, - Not suitable for Cancel Order, instead we show the order_id
}