
use anchor_lang::prelude::*;

use crate::{Order, OrderStatus,error::ErrorCode};

pub const MAX_OPEN_ORDERS:usize = 64;

#[derive(InitSpace)]
#[account]
pub struct OpenOrdersAccount{
    pub owner:Pubkey,
    pub market: Pubkey,
    pub bump:u8,
    pub orders : [Order;MAX_OPEN_ORDERS],
    // This is parallel array for fast lookup
    pub client_order_ids : [u64;MAX_OPEN_ORDERS],
}

impl OpenOrdersAccount {
    // returns index of the free slot 
    pub fn find_free_slot(&self)->Result<usize>{
        for (i,slot) in self.orders.iter().enumerate(){
            if slot.status == OrderStatus::FREE {
                return Ok(i);
            }
        }
        Err(error!(ErrorCode::MaxOrderReached))
    }

    // Find the index of the order by its client_id
    pub fn find_order_by_client_id(&self,client_order_id:u64)->Result<usize>{
        for (i,&id) in self.client_order_ids.iter().enumerate() {
            if id == client_order_id && self.orders[i].status != OrderStatus::FREE {
                return Ok(i)
            }
        }

        Err(error!(ErrorCode::OrderNotFound))
    }

    pub fn find_order_by_order_id(&self,order_id:u128)->Result<usize>{
        for (i , order) in self.orders.iter().enumerate() {
            if order.order_id == order_id{
                return Ok(i)
            }
        }

        Err(error!(ErrorCode::OrderNotFound))
    }
}