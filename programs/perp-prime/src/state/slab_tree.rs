use std::{arch::x86_64::_MM_FROUND_CUR_DIRECTION, env::current_dir};

use anchor_lang::prelude::{bpf_loader_upgradeable::id, *};

use crate::error::ErrorCode;

#[repr(C)]
#[derive(AnchorDeserialize,AnchorSerialize,Debug,Clone,Copy,Default)]
pub struct SlabHeader{
    pub root_index: u64,
    pub order_count: u64,
    pub bump_index:u64,  // next index to allocate
    pub free_list_len: u64,
    pub free_list_head: u64,  // index of the free node head
}

#[repr(C)]
#[derive(AnchorDeserialize,AnchorSerialize,Debug,Clone,Copy,Default,PartialEq)]
pub enum NodeType{
    #[default]
    UNINITIALIZED,
    INNER,
    FREE,
    LEAF,
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
    pub left:u64,
    pub right:u64,
    pub crit_bit:u8,
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

    fn header_size()->usize { std::mem::size_of::<SlabHeader>()}

    fn node_size()->usize {
        std::mem::size_of::<SlabNode>()
    }

    fn read_header(account_data:&[u8])->Result<SlabHeader>{
        let header_size = Self::header_size();
        Ok(
            SlabHeader::try_from_slice(&account_data[..header_size])?
        )
    }

    fn write_header(account_data:&mut [u8],header:&SlabHeader)->Result<()>{
        let header_size = Self::header_size();

        header.serialize(&mut &mut account_data[..header_size])?;
        
        Ok(())
    }

    // Read Node at a index (index must be >= 1)
    fn read_node(account_data:&[u8],index:u64)->Result<SlabNode>{
        if index == 0 {
        return Err(ErrorCode::IndexOutOfBound.into())
        }

        let header_size = Self::header_size();
        let node_size = Self::node_size();

        let offset = header_size + (index as usize)* node_size;

        let end = offset + node_size;

        if end > account_data.len() {
            return Err(ErrorCode::IndexOutOfBound.into())
        }

        Ok(SlabNode::try_from_slice(&account_data[offset..end])?)
    }

    // Write node to a index
    fn write_node(account_data:&mut[u8],index:u64,node:&SlabNode)->Result<()>{

        if index == 0 {
            return Err(ErrorCode::IndexOutOfBound.into())
        }

        let header_size = Self::header_size();
        let node_size = Self::node_size();

        let offset = header_size + (index as usize) * node_size;

        let end = offset + node_size;

        if end > account_data.len() {
            return Err(ErrorCode::IndexOutOfBound.into())
        }

        node.serialize(&mut &mut account_data[offset..end])?;

        Ok(())
    }

    fn allocate_leaf (account_data:&mut [u8],node:&SlabNode)->Result<u64>{
        let mut header = Self::read_header(account_data)?;

        let index:u64;

        if header.free_list_len > 0 {
            // get the head
            let free_idx = header.free_list_head;
            // we need to read the free node to get the next
            let free_node = Self::read_node(account_data, free_idx)?;

            header.free_list_head = free_node.next;
            header.free_list_len = header.free_list_len.checked_sub(1).ok_or(ErrorCode::MathError)?;
            index = free_idx;
            Self::write_node(account_data, index, node)?;
        }else{
            index = header.bump_index;
            Self::write_node(account_data, index, node)?;

            header.bump_index = header.bump_index.checked_add(1).ok_or(ErrorCode::IndexOutOfBound)?;
        }

        header.order_count = header.order_count.checked_add(1).ok_or(ErrorCode::AdditionOverflow)?;
        Self::write_header(account_data, &header);

        Ok(index)
    }


    // Allocate an index for the inner node
    fn allocate_index (account_data:&mut[u8])->Result<u64>{
        let mut header = Self::read_header(account_data)?;
        let idx: u64;

        if header.free_list_len > 0 {
            let free_idx = header.free_list_head;
            let free_node = Self::read_node(account_data, free_idx)?;
            header.free_list_head = free_node.next;
            header.free_list_len = header.free_list_len.checked_sub(1).ok_or(ErrorCode::MathError)?;
            idx = free_idx;
        }else{
            idx = header.bump_index;
            header.bump_index = header.bump_index.checked_add(1).ok_or(ErrorCode::IndexOutOfBound)?;
        }

        Self::write_header(account_data, &header)?;

        Ok(idx)
    }


    // It pushes the index to the free list , basically makes the nodes as Free Node
    fn push_free(account_data:&mut [u8],idx:u64)->Result<()>{
        if idx == 0 { return Err(ErrorCode::IndexOutOfBound.into()); }

        let mut header = Self::read_header(account_data)?;

        let free_node = SlabNode {
            tag: NodeType::FREE,
            key: 0,
            next: header.free_list_head,
            owner: Pubkey::default(),
            quantity: 0,
            order_id: 0,
            left: 0,
            right: 0,
            crit_bit: 0,
        };

        Self::write_node(account_data, idx, &free_node)?;

        header.free_list_head = idx;

        header.free_list_len = header.free_list_len.checked_add(1).ok_or(ErrorCode::MathError)?;

        Self::write_header(account_data, &header)?;

        Ok(())
    }

    // Get the bit value of the key at a specified position
    fn bit_of(key:u128,pos:u8)->u8 {
        ((key>>pos) & 1) as u8
    }

    fn get_msb_index(xor:u128)->u8{
        let lz = xor.leading_zeros();
        // index from LSB : 127-lz , we have used wrapping_sub, instead of paniking it wraps around
        (127u8).wrapping_sub(lz as u8)
    }

    // It find leaf and also its parent : returns (leaf_index,parent_index,direction_from_parent)
    // direction: 0 - left, 1 - right, parent_index = 0 means leaf is root.
    fn find_leaf_and_parent(account_data:&[u8],root_index:u64,key:u128)->Result<(u64,u64,u8)>{
        if root_index == 0{
            return Err(ErrorCode::IndexOutOfBound.into())
        }

        let mut parent: u64 = 0;
        let mut dir :u8 = 0;
        let mut current = root_index;

        loop {
            let node = Self::read_node(account_data, current)?;

            match node.tag {
                NodeType::INNER => {
                    // if inner , compare bit at node.crit_bit
                    let crit = node.crit_bit;
                    let bit = Self::bit_of(key, crit);
                    parent = current;
                    dir = bit;
                    current = if bit == 0 { node.left} else { node.right };

                    if current == 0 {
                        return Err(ErrorCode::IndexOutOfBound.into())
                    }
                }
                NodeType::LEAF =>{
                    return Ok((current,parent,dir))
                }
                _ =>{
                    return Err(ErrorCode::UnexpectedNodeTag.into())
                }
            }
        }
    }

    // This returns ancestor path : Vec<(node_index,dir)>
    // leaf is root - path = [] and leaf_idx = root_idx
    fn find_leaf_and_path(account_data:&[u8],root_index:u64,key:u128)->Result<(u64,Vec<(u64,u8)>)>{
        if root_index == 0 { return Err(ErrorCode::IndexOutOfBound.into()); }

        let mut path:Vec<(u64,u8)> = Vec::new();
        let mut current = root_index;

        loop{
            let node = Self::read_node(account_data, current)?;
            match node.tag {
                NodeType::INNER =>{
                    let b = Self::bit_of(key, node.crit_bit);
                    path.push((current,b));
                    current = if b == 0 { node.left} else { node.right};
                    if current == 0 { return Err(ErrorCode::IndexOutOfBound.into()); }
                }
                NodeType::LEAF => {
                    return Ok((current,path))
                },
                _ => return Err(ErrorCode::UnexpectedNodeTag.into()),
            }
        }
    }

    /// - Allocates a new leaf node (from free or bump)
    /// - If tree empty, set root = leaf
    /// - Otherwise: find existing leaf E where new leaf compares
    /// - compute new_crit_bit = msb_index(xor)
    /// - Find splice point by traversing until crit_bit <= new_crit_bit
    /// - Create inner node I with crit_bit, children assigned according to bit
    /// - Splice I into tree (update parent's child or root)
    pub fn insert(account_data:&mut [u8],key:u128,owner:Pubkey,qty:u64,order_id:u128)->Result<()>{

        let leaf_node = SlabNode{
            tag: NodeType::LEAF,
            key,
            next: 0,
            owner,
            quantity: qty,
            order_id,
            crit_bit: 0,
            left: 0,
            right: 0,
        };

        let mut header = Self::read_header(account_data)?;

        // if tree empty -> allocate leaf and set root
        if header.root_index == 0 {
            let idx = Self::allocate_leaf(account_data, &leaf_node)?;

            header.root_index = idx;

            Self::write_header(account_data, &header)?;
            return Ok(())
        }

        // Traverse and find the leaf with which we will compare with 
        let (existing_leaf_idx, _parent_of_e, _dir_of_e) = Self::find_leaf_and_parent(account_data, header.root_index, key)?;

        let existing_leaf = Self::read_node(account_data, existing_leaf_idx)?;

        if existing_leaf.key == key {
            return Err(error!(ErrorCode::DuplicateKey))
        }

        // critical bit calculation
        let xor = existing_leaf.key ^ key;

        if xor == 0 {
            // already handled but guard
            return Err(error!(ErrorCode::DuplicateKey));
        }

        let new_crit_bit = Self::get_msb_index(xor);

        // 2nd traversal, we have to find where to splice the new inner node
        // traverse until , new_crit_bit <= current Node Critbit
        let mut parent: u64 = 0;
        let mut dir : u8 = 0;
        let current = header.root_index;

        loop{
            let curr_node = Self::read_node(account_data, current)?;

            match curr_node.tag {
                NodeType::INNER => {

                    // if true, then we must insert above
                    if curr_node.crit_bit <= new_crit_bit {
                        break;
                    }

                    let b = Self::bit_of(key, curr_node.crit_bit);

                    parent = current;
                    dir = b;
                    current = if b == 0 {curr_node.left} else { curr_node.right };
                     
                    if current == 0 {
                        return Err(error!(ErrorCode::IndexOutOfBound));
                    }

                },
                NodeType::LEAF => {
                    // reached a leaf -> stop (we will splice above this leaf)
                    break;
                }
                _ => return Err(error!(ErrorCode::UnexpectedNodeTag)),
            }
        }

        // Allocate the new leaf so we can have its index
        let new_leaf_idx = Self::allocate_leaf(account_data, &leaf_node)?;

        // Read the node that will be the sibiling - it is the current node 
        let sibling_node = Self::read_node(account_data, current)?;
        let sibling_idx = current;

        let new_bit_of_key = Self::bit_of(key, new_crit_bit);

        let (left_idx, right_idx) = if new_bit_of_key == 0 {
            (new_leaf_idx,sibling_idx)
        }else{
            (sibling_idx,new_leaf_idx)
        };

        let inner_node = SlabNode {
            tag: NodeType::INNER,
            key: 0, // optional: not used for inner in this layout
            next: 0,
            owner: Pubkey::default(),
            quantity: 0,
            order_id: 0,
            crit_bit: new_crit_bit,
            left: left_idx,
            right: right_idx,
        };

        // now we need to allocate an index for the inner node 
        let inner_idx = {
            let mut header2 = Self::read_header(account_data)?;

            let idx: u64;

            if header2.free_list_len > 0 {
                let free_idx = header2.free_list_head;
                let free_node = Self::read_node(account_data, free_idx)?;

                header2.free_list_head = free_node.next;
                header.free_list_len = header.free_list_len.checked_sub(1).ok_or(ErrorCode::MathError)?;
                idx = free_idx;
                Self::write_node(account_data, idx, &inner_node)?
            }else{
                idx = header2.bump_index;
                Self::write_node(account_data, idx, &inner_node)?;
                header2.bump_index = header2.bump_index.checked_add(1).ok_or(error!(ErrorCode::IndexOutOfBounds))?;
            }
            Self::write_header(account_data, &header2)?;
            idx
        };

        // splice inner_idx into tree: attach it to parent (or make it root)
        if parent == 0 {
            // replace root
            header.root_index = inner_idx;
            // write header (we already updated header.bump when allocating)
            Self::write_header(account_data, &header)?;
        } else {
            // update parent.child[parent_dir] = inner_idx
            let mut parent_node = Self::read_node(account_data, parent)?;
            if dir == 0 {
                parent_node.left = inner_idx;
            } else {
                parent_node.right = inner_idx;
            }
            Self::write_node(account_data, parent, &parent_node)?;
        }

        Ok(())
    }
    
    // Returns Index + SlabNode
    pub fn find_min(account_data:&[u8])->Result<(u64,SlabNode)>{
        let header = Self::read_header(account_data)?;

        if header.root_index == 0 {
            return Err(error!(ErrorCode::EmptySlab))
        }

        let mut current = header.root_index;

        loop {
            let node = Self::read_node(account_data, current)?;

            match node.tag {
                NodeType::INNER => {
                    if node.left == 0 {
                        return Err(error!(ErrorCode::IndexOutOfBound))
                    }
                    current = node.left;
                }
                NodeType::LEAF =>{
                    return Ok((current,node))
                }
                _  => {
                    return Err(error!(ErrorCode::UnexpectedNodeTag))
                }
            }
        }
    }

    pub fn find_max(account_data:&[u8])->Result<(u64,SlabNode)>{
        let header = Self::read_header(account_data)?;

        if header.root_index == 0 {
            return Err(error!(ErrorCode::EmptySlab))
        }

        let mut current = header.root_index;

        loop{
            let node = Self::read_node(account_data, current)?;

            match node.tag {
                NodeType::INNER =>{
                    if node.left == 0 {
                        return Err(error!(ErrorCode::IndexOutOfBound))
                    }

                    current = node.left
                }
                NodeType::LEAF =>{
                    return Ok((current,node))
                }
                _=>{

                }
            }
        }
    }


    pub fn remove_by_key(account_data:&mut[u8],key:u128)->Result<SlabNode>{
        let header = Self::read_header(account_data)?;

        if header.root_index == 0 {
            return Err(error!(ErrorCode::EmptySlab))
        }

        let (leaf_idx,path) = Self::find_leaf_and_path(account_data, header.root_index, key)?;

        let leaf_node = Self::read_node(account_data, leaf_idx)?;

        if leaf_node.tag != NodeType::LEAF {
            return Err(error!(ErrorCode::UnexpectedNodeTag))
        }
        if leaf_node.key != key {
            return Err(error!(ErrorCode::KeyMisMatch))
        }

        // checking if leaf is root
        if path.is_empty() {
            Self::push_free(account_data, leaf_idx)?;
            let mut header2 = Self::read_header(account_data)?;
            header2.root_index = 0;
            header2.order_count = header.order_count.checked_sub(1).ok_or(ErrorCode::MathError)?;
            Self::write_header(account_data, &header2)?;
            return Ok(leaf_node);
        }

        // finding leaf parent, it is the last entry in the path 
        let (parent_idx,_dir_to_leaf) = *path.last().unwrap();
        let parent_node = Self::read_node(account_data, parent_idx)?;
        if parent_node.tag != NodeType::INNER {
            return Err(error!(ErrorCode::UnexpectedNodeTag));
        }

        let sibling_idx = if parent_node.left == leaf_idx {parent_node.right} else{parent_node.left};


        // Finding Grandparent if exists
        let (gp_idx,gp_dir) = if path.len() >= 2 {
            path[path.len() - 2]
        }else{
            (0u64,0u8)
        };

        // splice grandparent into grandparent or root
        if gp_idx == 0 {
            // as path is short that means the element will be root now 
            let mut header2 = Self::read_header(account_data)?;

            header2.root_index = sibling_idx;
            header2.order_count = header2.order_count.checked_sub(1).ok_or(ErrorCode::MathError)?;
            Self::write_header(account_data, &header2)?;
        }else{
            // Here gp_dir repersent, which side of the grandparent point to the parent
            let mut gp_node = Self::read_node(account_data, gp_idx)?;
            if gp_dir ==0 {
                gp_node.left = sibling_idx;
            }else {
                gp_node.right = sibling_idx;
            }

            Self::write_node(account_data, gp_idx, &gp_node)?;

            let mut header2 = Self::read_header(account_data)?;
            header2.order_count = header2.order_count.checked_sub(1).ok_or(ErrorCode::MathError)?;
            Self::write_header(account_data, &header2)?;
        }

        // free parent and leaf 
        Self::push_free(account_data, parent_idx)?;
        Self::push_free(account_data, leaf_idx)?;
        Ok(leaf_node)

    }

    fn remove_by_index(account_data:&mut [u8],leaf_idx:u64)->Result<SlabNode>{
        let node = Self::read_node(account_data, leaf_idx)?;

        if node.tag != NodeType::LEAF {
            return Err(error!(ErrorCode::UnexpectedNodeTag));
        }

        Self::remove_by_key(account_data, node.key)

    }

    pub fn pop_min(account_data:&mut [u8])->Result<SlabNode>{
        let (idx,_) = Self::find_min(account_data)?;
        Self::remove_by_index(account_data, idx)
    }

    pub fn pop_max(account_data:&mut [u8])->Result<SlabNode>{
        let (idx,_) = Self::find_max(account_data)?;
        Self::remove_by_index(account_data, idx)
    }

}