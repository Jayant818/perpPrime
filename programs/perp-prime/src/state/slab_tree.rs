use anchor_lang::prelude::*;

#[repr(C)]
#[derive(AnchorDeserialize,AnchorSerialize,Debug,Clone,Copy,Default)]
pub struct SlabHeader{
    pub root_index: u64,
    pub order_count: u64,
    pub bump_index:u64,
    pub free_list_len: u64,
    pub free_list_head: u64,
}

#[repr(C)]
#[derive(AnchorDeserialize,AnchorSerialize,Debug,Clone,Copy,Default,PartialEq)]
pub enum NodeType{
    INNER,
    FREE,
    LEAF,
    #[default]
    UNINITIALIZED,
}


// For free node - Tag , next 
// For Inner node - Tag , key , order_id(prefix len), leaf_left , leaf_right
// For Leaf node - Tag , key , owner , quantity , order_id 
// For lead node the key and order_id both are same, 
// For Inner Node - Common Prefix bits for Key , Prefix Length -  order_id
#[repr(C)]
#[derive(AnchorDeserialize,AnchorSerialize,Debug,Clone,Default)]
pub struct SlabNode{
    pub tag:NodeType,
    pub key:u128,  // sorting key - price+seq , this is for the ordering
    pub next:u64, // used in free nodes , index of next free node(linked  list)
    pub owner: Pubkey,
    pub quantity:u64,
    pub order_id:u128, // used for user facing logic, like cancel/lookup etc
    pub leaf_left: u8, // left child index (for inner)
    pub leaf_right:u8, // right child index (for inner)
}

pub struct Slab;

impl Slab{
    pub fn initialize(account_data:&mut [u8])->Result<()>{

        let header = SlabHeader{
            root_index :0,
            bump_index: 1,
            free_list_head:0,
            free_list_len:0,
            order_count:0
        };

        header.serialize(&mut &mut account_data[..std::mem::size_of::<SlabHeader>()])?;

        Ok(())
    }

    pub fn insert(account_data:&mut [u8],key:u128,owner:Pubkey,qty:u64,order_id:u128)->Result<()>{

        let header_size = std::mem::size_of::<SlabHeader>();
        let mut header = SlabHeader::try_from_slice(&account_data[..header_size])?;
        
        let node_size = std::mem::size_of::<SlabNode>();    
        let node = SlabNode{
            tag: NodeType::LEAF,
            key,
            order_id,
            owner,
            quantity:qty,
            ..Default::default()
        };
        // CHECK if there is some available some free node, but there is some problem in this approach we have to create inner node also somehow?
        // For Creating the Inner node I have to compare two keys

        let offset : usize;

        if header.free_list_len > 0 {
            //  so we need to serialize at that free node 
            offset = header_size + (header.free_list_head as usize * node_size);

            // Getting the free node to get the next value 
            let free_node = SlabNode::try_from_slice(&account_data[offset..offset+node_size])?;

            header.free_list_head = free_node.next;

            node.serialize(&mut &mut account_data[offset..offset+node_size])?;

            header.free_list_len -=1;

            
        }else {
            let new_index = header.bump_index;


            offset = header_size + (new_index as usize * node_size);

            node.serialize(&mut &mut account_data[offset..offset+node_size])?;

            header.order_count +=1;
            header.bump_index +=1;
            header.root_index = new_index;

        }
        header.serialize(&mut &mut account_data[..header_size])?;

        // Finding the same level child to get the key of that 

        let node_at_same_level = SlabNode::try_from_slice(&account_data[offset-node_size..offset])?;

        // we have to check if node_at_same_level is a Inner Node or not , If it not a inner_node then we will create a inner node otherwise not cuz , inner node basically compares 2 leaf nodes

        if node_at_same_level.tag == NodeType::INNER {
            return Ok(())
        }

        let key1 = node_at_same_level.key;
        let key2 = node.key;

        // Somehow we have to find out the critical bit here so that we can fill the inner Node 
        let res = std::cmp::max_by_key(key1, key2);

        let inner_node = SlabNode{
            tag: NodeType::INNER,
            key :res,
            leaf_left : node_at_same_level.key, // or either we need to store the index of that - offset-node_size
            leaf_right: node.key, // or either we have to store the offset,
            order_id: node.key[..res],
            ..Default::default()
        };

        inner_node.serialize(&mut &mut account_data[offset+node_size..offset+node_size+node_size])?;

        // As we have inserted one more node so we have increased the bump_index.
        header.bump_index+=1;
        header.serialize(&mut &mut account_data[..header_size])?;

        Ok(())
    }
    
}