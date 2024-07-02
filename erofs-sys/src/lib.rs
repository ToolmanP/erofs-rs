#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

pub type Blk = u32;
pub type Offset = u64;
pub type Nid = u64;

mod superblock;
mod compression;
mod inode;
mod dir;
