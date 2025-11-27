use bytemuck::{Pod, Zeroable, checked::cast};

use crate::{OrderPosition, OrderSide, OrderType};

#[derive(PartialEq)]
#[repr(u8)]
pub enum OrderStatus{
    FREE = 0, // -> the initial state of all nodes
    PENDING = 1, // -> sent to the request_queue, not yet on the book
    OPEN = 2,  // on the slab
    FILLED = 3, // completely filled
    CANCELLED = 4, // sent to the request queue, not yet removed from book
}

#[derive(Clone, Copy,Debug,Zeroable,Pod)]
#[repr(C)]
pub struct Order{
    pub status: u8,
    pub side: u8,
    pub order_type: u8,
    pub position:u8,
    pub is_liquidating:u8,
    pub _padding: [u8;3],
    pub quantity: u64,
    pub order_id :[u8;16],
    // Order Id provided by the client for easier lookup,
    pub client_order_id:u64,
    pub locked_margin: u64,
    pub limit_price:u64,
}

#[cfg(feature = "idl-build")]
impl anchor_lang::IdlBuild for Order {}


impl Order {

    pub fn new(
        status:OrderStatus,
        side:OrderSide,
        order_type:OrderType,
        position:OrderPosition,
        is_liquidating: bool,
        quantity:u64,
        order_id:u128,
        client_order_id:u64,
        locked_margin:u64,
        limit_price:u64,
    )->Self{
        Self { 
            status: status as u8, 
            side: side as u8, 
            order_type: order_type as u8, 
            position: position as u8, 
            is_liquidating: is_liquidating as u8, 
            _padding: [0;3], 
            quantity: quantity, 
            order_id: cast(order_id), 
            client_order_id: client_order_id, 
            locked_margin: locked_margin, 
            limit_price: limit_price 
        }
    }

    pub fn get_position_status(&self)->OrderPosition{
        match self.position{
            0=> OrderPosition::LONG,
            1=> OrderPosition::SHORT,
            _=> OrderPosition::LONG
        }
    }

    pub fn set_position_status(&mut self, position:OrderPosition){
        self.position = position as u8;
    }

    pub fn set_status(&mut self,status:OrderStatus){
        self.status = status as u8;
    }

    pub fn get_status(&self)->OrderStatus{
        match self.status {
            0=> OrderStatus::FREE,
            1=> OrderStatus::PENDING,
            2=> OrderStatus::OPEN,
            3=> OrderStatus::FILLED,
            4=> OrderStatus::CANCELLED,
            _=> OrderStatus::FREE
        }
    }

    pub fn set_side(&mut self,side:OrderSide){
        self.side = side as u8;
    }

    pub fn get_side(&self)->OrderSide{
        match self.side {
            0 => OrderSide::BID,
            1=> OrderSide::ASK,
            _=> OrderSide::BID
        }
    }
}