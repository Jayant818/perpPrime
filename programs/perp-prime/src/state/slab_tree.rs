use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable, bytes_of, from_bytes};
use crate::error::ErrorCode;

pub const NODE_SIZE:usize = 128; 

#[repr(C)]
#[derive(Pod,Zeroable,Debug,Clone,Copy,Default)]
pub struct SlabHeader{
    pub root_index: u64,
    pub order_count: u64,
    pub bump_index:u64,  // next index to allocate
    pub free_list_len: u64,
    pub free_list_head: u64,  // index of the free node head
}

const _:() = assert!(std::mem::size_of::<SlabHeader>() == 40);

pub mod node_tag {
    pub const UNINIT: u8 = 0;
    pub const INNER: u8 = 1;
    pub const FREE: u8 = 2;
    pub const LEAF: u8 = 3;
}

// For free node - Tag , next 
// For Inner node - Tag , key , order_id(prefix len), leaf_left , leaf_right
// For Leaf node - Tag , key , owner , quantity , order_id 
// For lead node the key and order_id both are same, 
// For Inner Node - Common Prefix bits for Key , Prefix Length -  order_id
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct SlabNode {
    pub tag: u8,               // 1
    pub padding0: [u8; 15],     // 7 -> align to 8
    pub key: u128,             // 16
    pub next: u64,             // 8  (for free list)
    pub owner: [u8; 32],       // 32 (pubkey bytes)
    pub quantity: u64,         // 8
    pub order_id: u128,        // 16
    pub left: u64,             // 8
    pub right: u64,            // 8
    pub crit_bit: u8,          // 1
    pub padding1: [u8; 15],    // pad to NODE_SIZE (total = 128)
}

const _: () = assert!(std::mem::size_of::<SlabNode>() == NODE_SIZE);

pub struct Slab;

impl Slab{

    fn header_size()->usize { std::mem::size_of::<SlabHeader>()}

    fn node_size() -> usize { NODE_SIZE }

    pub fn initialize(account_data:&mut [u8])->Result<()>{

        let header = SlabHeader{
            root_index :0,
            bump_index: 1,
            free_list_head:0,
            free_list_len:0,
            order_count:0
        };

        let header_size = Self::header_size();
        require!(account_data.len() >= header_size, ErrorCode::AccountDataTooSmall);

        let header_bytes = bytes_of(&header);

        account_data[..header_size].copy_from_slice(header_bytes);

        Ok(())
    }

    pub fn read_header(account_data:&[u8])->Result<SlabHeader>{
        let header_size = Self::header_size();
        let header_ref = from_bytes(&account_data[..header_size]);
        Ok(*header_ref)
    }

    pub fn write_header(account_data:&mut [u8],header:&SlabHeader)->Result<()>{
        let header_size = Self::header_size();

        account_data[..header_size].copy_from_slice(bytes_of(header));
        
        Ok(())
    }

    fn node_offset(index:u64)->usize{
        let hs = Self::header_size();
        hs+(index as usize).saturating_mul(Self::node_size())
    }

    pub fn read_node(account_data:&[u8],index:u64)->Result<SlabNode>{
        if index == 0 {
            return Err(ErrorCode::IndexOutOfBound.into())
        }

        let off = Self::node_offset(index);
        let ns = Self::node_size();
        let end = off.checked_add(ns).ok_or(ErrorCode::IndexOutOfBound)?;
        require!(end <= account_data.len(), ErrorCode::IndexOutOfBound);

        let node_ref:&SlabNode = from_bytes(&account_data[off..end]);
       
       Ok(*node_ref)
    }

    // Write node to a index
    pub fn write_node(account_data: &mut [u8], index: u64, node: &SlabNode) -> Result<()> {
        if index == 0 { return Err(ErrorCode::IndexOutOfBound.into()); }
        let off = Self::node_offset(index);
        let ns = Self::node_size();
        let end = off.checked_add(ns).ok_or(ErrorCode::IndexOutOfBound)?;
        require!(end <= account_data.len(), ErrorCode::IndexOutOfBound);
        account_data[off..end].copy_from_slice(bytemuck::bytes_of(node));
        Ok(())
    }

    fn allocate_leaf(account_data: &mut [u8], node: &SlabNode) -> Result<u64> {
        let mut header = Self::read_header(account_data)?;

        let idx: u64;

        if header.free_list_len > 0 {

            let free_idx = header.free_list_head;

            let free_node = Self::read_node(account_data, free_idx)?;

            header.free_list_head = free_node.next;

            header.free_list_len = header.free_list_len.checked_sub(1).ok_or(ErrorCode::MathError)?;

            idx = free_idx;

            Self::write_node(account_data, idx, node)?;

        } else {
            idx = header.bump_index;

            Self::write_node(account_data, idx, node)?;

            header.bump_index = header.bump_index.checked_add(1).ok_or(ErrorCode::IndexOutOfBound)?;
        }

        header.order_count = header.order_count.checked_add(1).ok_or(ErrorCode::AdditionOverflow)?;

        Self::write_header(account_data, &header)?;

        Ok(idx)
    }



    // Allocate an index for the inner node
    fn allocate_index(account_data: &mut [u8]) -> Result<u64> {
        let mut header = Self::read_header(account_data)?;
        let idx: u64;
        if header.free_list_len > 0 {
            let free_idx = header.free_list_head;
            let free_node = Self::read_node(account_data, free_idx)?;
            header.free_list_head = free_node.next;
            header.free_list_len = header.free_list_len.checked_sub(1).ok_or(ErrorCode::MathError)?;
            idx = free_idx;
        } else {
            idx = header.bump_index;
            header.bump_index = header.bump_index.checked_add(1).ok_or(ErrorCode::IndexOutOfBound)?;
        }
        Self::write_header(account_data, &header)?;
        Ok(idx)
    }


    // It pushes the index to the free list , basically makes the nodes as Free Node
    fn push_free(account_data: &mut [u8], idx: u64) -> Result<()> {
        if idx == 0 { return Err(ErrorCode::IndexOutOfBound.into()); }
        let mut header = Self::read_header(account_data)?;

        let free_node = SlabNode {
            tag: node_tag::FREE,
            padding0: [0u8; 15],
            key: 0,
            next: header.free_list_head,
            owner: [0u8; 32],
            quantity: 0,
            order_id: 0,
            left: 0,
            right: 0,
            crit_bit: 0,
            padding1: [0u8; 15],
        };

        Self::write_node(account_data, idx, &free_node)?;

        header.free_list_head = idx;
        header.free_list_len = header.free_list_len.checked_add(1).ok_or(ErrorCode::MathError)?;
        Self::write_header(account_data, &header)?;
        Ok(())
    }

    // Get the bit value of the key at a specified position
    #[inline(always)]
    fn bit_of(key: u128, pos: u8) -> u8 {
        ((key >> pos) & 1) as u8
    }

    #[inline(always)]
    fn get_msb_index(xor: u128) -> u8 {
        let lz = xor.leading_zeros();
        (127u8).wrapping_sub(lz as u8)
    }

    // It find leaf and also its parent : returns (leaf_index,parent_index,direction_from_parent)
    // direction: 0 - left, 1 - right, parent_index = 0 means leaf is root.
    fn find_leaf_and_parent(account_data: &[u8], root_index: u64, key: u128) -> Result<(u64, u64, u8)> {
        if root_index == 0 { return Err(ErrorCode::IndexOutOfBound.into()); }
        let mut parent: u64 = 0;
        let mut dir: u8 = 0;
        let mut current = root_index;

        loop {
            let node = Self::read_node(account_data, current)?;
            match node.tag {
                x if x == node_tag::INNER => {
                    let crit = node.crit_bit;
                    let bit = Self::bit_of(key, crit);
                    parent = current;
                    dir = bit;
                    current = if bit == 0 { node.left } else { node.right };
                    if current == 0 { return Err(ErrorCode::IndexOutOfBound.into()); }
                }
                x if x == node_tag::LEAF => return Ok((current, parent, dir)),
                _ => return Err(ErrorCode::UnexpectedNodeTag.into()),
            }
        }
    }

    // This returns ancestor path : Vec<(node_index,dir)>
    // leaf is root - path = [] and leaf_idx = root_idx
    fn find_leaf_and_path(account_data: &[u8], root_index: u64, key: u128) -> Result<(u64, Vec<(u64, u8)>)> {
        if root_index == 0 { return Err(ErrorCode::IndexOutOfBound.into()); }
        let mut path: Vec<(u64, u8)> = Vec::new();
        let mut current = root_index;

        loop {
            let node = Self::read_node(account_data, current)?;
            match node.tag {
                x if x == node_tag::INNER => {
                    let b = Self::bit_of(key, node.crit_bit);
                    path.push((current, b));
                    current = if b == 0 { node.left } else { node.right };
                    if current == 0 { return Err(ErrorCode::IndexOutOfBound.into()); }
                }
                x if x == node_tag::LEAF => return Ok((current, path)),
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
    pub fn insert(account_data: &mut [u8], key: u128, owner: Pubkey, qty: u64, order_id: u128) -> Result<()> {
        let owner_bytes: [u8; 32] = owner.to_bytes();
        let leaf_node = SlabNode {
            tag: node_tag::LEAF,
            padding0: [0u8; 15],
            key,
            next: 0,
            owner: owner_bytes,
            quantity: qty,
            order_id,
            left: 0,
            right: 0,
            crit_bit: 0,
            padding1: [0u8; 15],
        };

        let mut header = Self::read_header(account_data)?;

        // empty tree
        if header.root_index == 0 {
            let idx = Self::allocate_leaf(account_data, &leaf_node)?;
            header.root_index = idx;
            Self::write_header(account_data, &header)?;
            return Ok(());
        }

        // find leaf to compare
        let (existing_leaf_idx, _parent_of_e, _dir_of_e) =
            Self::find_leaf_and_parent(account_data, header.root_index, key)?;

        let existing_leaf = Self::read_node(account_data, existing_leaf_idx)?;
        if existing_leaf.key == key { return Err(error!(ErrorCode::DuplicateKey)); }

        let xor = existing_leaf.key ^ key;
        if xor == 0 { return Err(error!(ErrorCode::DuplicateKey)); }

        let new_crit_bit = Self::get_msb_index(xor);

        // second traversal to find splice point
        let mut parent: u64 = 0;
        let mut dir: u8 = 0;
        let mut current = header.root_index;

        loop {
            let curr_node = Self::read_node(account_data, current)?;
            match curr_node.tag {
                x if x == node_tag::INNER => {
                    if curr_node.crit_bit <= new_crit_bit {
                        break;
                    }
                    let b = Self::bit_of(key, curr_node.crit_bit);
                    parent = current;
                    dir = b;
                    current = if b == 0 { curr_node.left } else { curr_node.right };
                    if current == 0 { return Err(error!(ErrorCode::IndexOutOfBound)); }
                }
                x if x == node_tag::LEAF => break,
                _ => return Err(error!(ErrorCode::UnexpectedNodeTag)),user
            }
        }

        // allocate new leaf
        let new_leaf_idx = Self::allocate_leaf(account_data, &leaf_node)?;
        let sibling_idx = current;
        let new_bit_of_key = Self::bit_of(key, new_crit_bit);
        let (left_idx, right_idx) = if new_bit_of_key == 0 {
            (new_leaf_idx, sibling_idx)
        } else {
            (sibling_idx, new_leaf_idx)
        };

        let inner_node = SlabNode {
            tag: node_tag::INNER,
            padding0: [0u8; 15],
            key: 0,
            next: 0,
            owner: [0u8; 32],
            quantity: 0,
            order_id: 0,
            left: left_idx,
            right: right_idx,
            crit_bit: new_crit_bit,
            padding1: [0u8; 15],
        };

        // allocate index for inner node and write it
        let inner_idx = Self::allocate_index(account_data)?;
        Self::write_node(account_data, inner_idx, &inner_node)?;

        // splice into tree
        if parent == 0 {
            header.root_index = inner_idx;
            Self::write_header(account_data, &header)?;
        } else {
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
    pub fn find_min(account_data: &[u8]) -> Result<(u64, SlabNode)> {
        let header = Self::read_header(account_data)?;
        if header.root_index == 0 { return Err(error!(ErrorCode::EmptySlab)); }
        let mut current = header.root_index;
        loop {
            let node = Self::read_node(account_data, current)?;
            match node.tag {
                x if x == node_tag::INNER => {
                    if node.left == 0 { return Err(error!(ErrorCode::IndexOutOfBound)); }
                    current = node.left;
                }
                x if x == node_tag::LEAF => return Ok((current, node)),
                _ => return Err(error!(ErrorCode::UnexpectedNodeTag)),
            }
        }
    }

    pub fn find_max(account_data: &[u8]) -> Result<(u64, SlabNode)> {
        let header = Self::read_header(account_data)?;
        if header.root_index == 0 { return Err(error!(ErrorCode::EmptySlab)); }
        let mut current = header.root_index;
        loop {
            let node = Self::read_node(account_data, current)?;
            match node.tag {
                x if x == node_tag::INNER => {
                    if node.right == 0 { return Err(error!(ErrorCode::IndexOutOfBound)); }
                    current = node.right;
                }
                x if x == node_tag::LEAF => return Ok((current, node)),
                _ => return Err(error!(ErrorCode::UnexpectedNodeTag)),
            }
        }
    }


    pub fn remove_by_key(account_data: &mut [u8], key: u128) -> Result<SlabNode> {
        let header = Self::read_header(account_data)?;
        if header.root_index == 0 { return Err(error!(ErrorCode::EmptySlab)); }

        let (leaf_idx, path) = Self::find_leaf_and_path(account_data, header.root_index, key)?;
        let leaf_node = Self::read_node(account_data, leaf_idx)?;
        if leaf_node.tag != node_tag::LEAF { return Err(error!(ErrorCode::UnexpectedNodeTag)); }
        if leaf_node.key != key { return Err(error!(ErrorCode::KeyMisMatch)); }

        // if leaf is root
        if path.is_empty() {
            Self::push_free(account_data, leaf_idx)?;
            let mut header2 = Self::read_header(account_data)?;
            header2.root_index = 0;
            header2.order_count = header2.order_count.checked_sub(1).ok_or(ErrorCode::MathError)?;
            Self::write_header(account_data, &header2)?;
            return Ok(leaf_node);
        }

        // parent is last entry in path
        let (parent_idx, _dir_to_leaf) = path[path.len() - 1];
        let parent_node = Self::read_node(account_data, parent_idx)?;
        if parent_node.tag != node_tag::INNER { return Err(error!(ErrorCode::UnexpectedNodeTag)); }

        let sibling_idx = if parent_node.left == leaf_idx { parent_node.right } else { parent_node.left };

        // grandparent info if exists
        let (gp_idx, gp_dir) = if path.len() >= 2 { path[path.len() - 2] } else { (0u64, 0u8) };

        // splice: if no grandparent -> sibling becomes root; else attach to grandparent
        if gp_idx == 0 {
            let mut header2 = Self::read_header(account_data)?;
            header2.root_index = sibling_idx;
            header2.order_count = header2.order_count.checked_sub(1).ok_or(ErrorCode::MathError)?;
            Self::write_header(account_data, &header2)?;
        } else {
            let mut gp_node = Self::read_node(account_data, gp_idx)?;
            if gp_dir == 0 {
                gp_node.left = sibling_idx;
            } else {
                gp_node.right = sibling_idx;
            }
            Self::write_node(account_data, gp_idx, &gp_node)?;
            let mut header2 = Self::read_header(account_data)?;
            header2.order_count = header2.order_count.checked_sub(1).ok_or(ErrorCode::MathError)?;
            Self::write_header(account_data, &header2)?;
        }

        // free parent and leaf slots
        Self::push_free(account_data, parent_idx)?;
        Self::push_free(account_data, leaf_idx)?;
        Ok(leaf_node)
    }
 
    pub fn remove_by_index(account_data: &mut [u8], leaf_idx: u64) -> Result<SlabNode> {
        let node = Self::read_node(account_data, leaf_idx)?;
        if node.tag != node_tag::LEAF { return Err(error!(ErrorCode::UnexpectedNodeTag)); }
        Self::remove_by_key(account_data, node.key)
    }

    pub fn pop_min(account_data: &mut [u8]) -> Result<SlabNode> {
        let (idx, _) = Self::find_min(account_data)?;
        Self::remove_by_index(account_data, idx)
    }

    pub fn pop_max(account_data: &mut [u8]) -> Result<SlabNode> {
        let (idx, _) = Self::find_max(account_data)?;
        Self::remove_by_index(account_data, idx)
    }

    pub fn get_price_from_key(key: u128) -> u64 { (key >> 64) as u64 }

}

