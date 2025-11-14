use anchor_lang::prelude::*;
use crate::error::ErrorCode;
// Meta Data stored at the start of each queue account
#[repr(C)] 
#[derive(AnchorDeserialize,AnchorSerialize,Debug,Clone,Default)]
pub struct QueueHeader{
    pub head: u64,
    pub tail: u64,
    pub capacity: u64,
    pub count: u64,
}

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
where  T : AnchorDeserialize + AnchorSerialize + Clone + Default,
{
    pub fn intialize(account_data:&mut[u8],capacity:usize)->Result<()>{
        let mut header = QueueHeader::default();
        header.initialize(capacity as u64);

        // write header at the start - Serialize header into the first N bytes of account_data  - first 32 bytes are changed
        header.serialize(&mut &mut account_data[..std::mem::size_of::<QueueHeader>()])?;

        Ok(())
    }

    pub fn push(account_data:&mut[u8],item:&T)->Result<()>{

        let header_size = std::mem::size_of::<QueueHeader>();
        let mut header:QueueHeader = QueueHeader::try_from_slice(&account_data[..header_size])?;

        require!(header.count<header.capacity, ErrorCode::QueueIsFull);

        let item_size = std::mem::size_of::<T>();
        let offset = header_size + (header.tail as usize + item_size);
        item.serialize(&mut &mut account_data[offset..offset+item_size])?;

        // updating header
        header.tail = (header.tail+1)%header.capacity;
        header.count +=1;
        header.serialize(&mut &mut account_data[..header_size])?;

        Ok(())
    }

    // Pop one item from the queue, return None if Empty 
    pub fn pop(account_data:&mut[u8])->Result<Option<T>>{

        let header_size = std::mem::size_of::<QueueHeader>();
        let mut header:QueueHeader = QueueHeader::try_from_slice(&account_data[..header_size])?;

        if header.count == 0 {
            return Ok(None)
        }

        let item_size = std::mem::size_of::<T>();
        //  suppose header is a 4th index then doing, so finding the header.
        let offset = header_size + (header.head as usize * item_size);

        let item = T::try_from_slice(&account_data[offset..offset+item_size])?;

        // update header 
        header.head = (header.head+1)%header.capacity;
        header.count-=1;

        header.serialize(&mut &mut account_data[..header_size])?;

        Ok(Some(item))
    }


    pub fn peek(account_data:&mut[u8])->Result<Option<T>>{
        let header_size = std::mem::size_of::<QueueHeader>();
        let header = QueueHeader::try_from_slice(&account_data[..header_size])?;

        if header.count == 0{
            return Ok(None)
        }

        let item_size = std::mem::size_of::<T>();

        let offset = header_size + (header.head as usize * item_size);

        let item = T::try_from_slice(&account_data[offset..offset+item_size])?;

        Ok(Some(item))
    }
}
