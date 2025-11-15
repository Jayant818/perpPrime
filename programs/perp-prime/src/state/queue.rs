use anchor_lang::prelude::*;
use crate::error::ErrorCode;
use bytemuck::{
    Pod,
    Zeroable,
    bytes_of,
    from_bytes,
    from_bytes_mut,
};

const DATA_OFFSET:usize = 0;
// Meta Data stored at the start of each queue account
#[repr(C)] 
#[derive(Copy,Clone,Debug,Pod,Zeroable)]
pub struct QueueHeader{
    pub head: u64,
    pub tail: u64,
    pub capacity: u64,
    pub count: u64,
}

// compile time check
const _:() = assert!(std::mem::size_of::<QueueHeader>()==32);

impl QueueHeader{
    pub fn initialize(&mut self,capacity:u64){
        self.head = 0;
        self.count = 0;
        self.tail = 0;
        self.capacity = capacity;
    }
}

pub struct CircularQueue<T>{
    phantom: std::marker::PhantomData<T>,
}

impl<T> CircularQueue<T> 
where  T : Pod+Zeroable+Copy,
{
    pub fn intialize(account_data:&mut[u8],capacity:usize)->Result<()>{
        let header_size = std::mem::size_of::<QueueHeader>();

        if account_data.len() < DATA_OFFSET + header_size {
            return Err(error!(ErrorCode::AccountDataTooSmall))
        }

        // This creates a instance where each byte is zero
        let mut header = QueueHeader::zeroed();
        header.initialize(capacity as u64);

        let header_bytes = bytes_of(&header);
        // it copies header_bytes into the account buffer
        account_data[DATA_OFFSET..DATA_OFFSET+header_size].copy_from_slice(header_bytes);
 
        Ok(())
    }

    pub fn push(account_data:&mut[u8],item:&T)->Result<()>{

       let header_size = std::mem::size_of::<QueueHeader>();
       let node_size = std::mem::size_of::<T>();

       if account_data.len() < DATA_OFFSET + header_size {
         return Err(error!(ErrorCode::AccountDataTooSmall))
       }

       // First, read header to get the tail position
       let header_slice = &account_data[DATA_OFFSET..DATA_OFFSET+header_size];
       let header_ref : &QueueHeader = from_bytes(header_slice);

       require!(header_ref.count < header_ref.capacity, ErrorCode::QueueIsFull);

       // computing offset for item slot 
        let slot_index = header_ref.tail as usize;
        let offset = DATA_OFFSET + header_size + slot_index * node_size;
        let end = offset + node_size;

        if end > account_data.len() {
            return Err(error!(ErrorCode::AccountDataTooSmall))
        }

        // copy item directly into the account buffer 
        let item_bytes = bytes_of(item);
        account_data[offset..end].copy_from_slice(item_bytes);

        // Now update header fields
        let header_slice = &mut account_data[DATA_OFFSET..DATA_OFFSET+header_size];
        let header_ref : &mut QueueHeader = from_bytes_mut(header_slice);
        header_ref.tail = (header_ref.tail + 1)% header_ref.capacity;
        header_ref.count +=1;

        Ok(())
    }

    // Pop one item from the queue, return None if Empty 
    pub fn pop(account_data:&mut[u8])->Result<Option<T>>{

        let header_size = std::mem::size_of::<QueueHeader>();
        let node_size = std::mem::size_of::<T>();

        // First read header to check if queue is empty
        let header_slice = &account_data[DATA_OFFSET..DATA_OFFSET+header_size];
        let header_ref: &QueueHeader = from_bytes(header_slice);

        if header_ref.count == 0 {
            return Ok(None);
        }

        let slot_index = header_ref.head as usize;
        let offset = DATA_OFFSET + header_size + slot_index*node_size;
        let end = offset + node_size;

        if end > account_data.len() {
            return Err(error!(ErrorCode::AccountDataTooSmall))
        }

        // Read the item before updating header
        let item_ref = from_bytes(&account_data[offset..end]);
        let item_copy : T = *item_ref;

        // Now update header
        let header_slice = &mut account_data[DATA_OFFSET..DATA_OFFSET+header_size];
        let header_ref: &mut QueueHeader = from_bytes_mut(header_slice);
        header_ref.count -=1;
        header_ref.head = (header_ref.head + 1) % header_ref.capacity;

        Ok(Some(item_copy))
        
    }


    pub fn peek(account_data:&mut[u8])->Result<Option<T>>{
        let header_size = std::mem::size_of::<QueueHeader>();
        let node_size = std::mem::size_of::<T>();

        let header_bytes = &mut account_data[DATA_OFFSET..DATA_OFFSET+header_size];
        let header_ref : &QueueHeader = from_bytes_mut(header_bytes);

        if header_ref.count == 0 {
            return Ok(None)
        }

        let slot_index = header_ref.head as usize;
        let offset = DATA_OFFSET + header_size + slot_index * node_size;
        let end = offset + node_size;

        if end > account_data.len() {
            return Err(error!(ErrorCode::AccountDataTooSmall));
        }


        let item_ref:&T = from_bytes(&account_data[offset..end]);
        Ok(Some(*item_ref))
    }
}
