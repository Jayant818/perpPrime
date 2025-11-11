use anchor_lang::prelude::{borsh::de, *};

#[derive(Clone,Copy,AnchorDeserialize,AnchorSerialize,Default)]
#[repr(C)]
pub enum OrderType{
    #[default]
    MarketOrder,
    LimitOrder
}

#[repr(C)]
#[derive(Clone,Copy,Debug,AnchorDeserialize,AnchorSerialize,Default)]
pub enum OrderSide{
    #[default]
    LONG,
    SHORT
}

#[repr(C)]
#[derive(Clone,Copy,AnchorDeserialize,AnchorSerialize,Default)]
pub enum RequestType{
    #[default]
    OPEN,
    CANCEL
}

#[repr(C)]
#[derive(AnchorDeserialize,AnchorSerialize,Clone,Default)]
pub struct RequestQueue{
    pub request_type:RequestType,
    pub order_type: OrderType,
    pub order_side :OrderSide,
    pub quantity: u64,
    pub user: Pubkey,
    pub order_id: u64,
    // pub entry_price: u64, - Not suitable for Cancel Order, instead we show the order_id
}