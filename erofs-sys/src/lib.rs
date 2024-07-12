#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

pub(crate) const EROFS_BLOCK_SZ: usize = 4096;
pub(crate) const EROFS_EMPTY_BLOCK: Block = [0; EROFS_BLOCK_SZ];
pub(crate) const EROFS_SUPER_OFFSET: Off = 1024;

pub type Block = [u8; EROFS_BLOCK_SZ];
pub type Blk = u32;
pub type Off = u64;
pub type Nid = u64;

mod compression;
mod data;
mod dir;
mod map;
mod inode;
mod superblock;

#[macro_export]
macro_rules! round {
    (UP, $x: expr, $y: expr) => {
        ($x + $y - 1) / $y * $y
    };
    (DOWN, $x: expr, $y: expr) => {
        ($x / $y) * $y
    };
}
