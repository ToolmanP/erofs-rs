#![no_std]
// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

pub(crate) const EROFS_BLOCK_SZ: u64 = 4096;
pub(crate) const EROFS_EMPTY_BLOCK: Block = [0; EROFS_BLOCK_SZ as usize];
pub(crate) const EROFS_SUPER_OFFSET: Off = 1024;
pub(crate) const EROFS_BLOCK_BITS: u64 = 12;
pub(crate) const EROFS_BLOCK_MASK: u64 = EROFS_BLOCK_SZ - 1;

pub(crate) struct PageAddress {
    pub(crate) page: u64,
    pub(crate) pg_off: u64,
    pub(crate) pg_len: u64,
}

impl From<u64> for PageAddress {
    fn from(address: u64) -> Self {
        PageAddress {
            page: (address >> EROFS_BLOCK_BITS) << EROFS_BLOCK_BITS,
            pg_off: address & EROFS_BLOCK_MASK,
            pg_len: EROFS_BLOCK_SZ - (address & EROFS_BLOCK_MASK),
        }
    }
}

// It's unavoidable to import alloc here. Since there are so many backends there and if we want to
// to use trait object to export Filesystem pointer. The alloc crate here is necessary.
extern crate alloc;

pub type Block = [u8; EROFS_BLOCK_SZ as usize];
pub type Blk = u32;
pub type Off = u64;
pub type Nid = u64;

mod compression;
mod data;
mod dir;
mod inode;
mod map;
mod operations;
mod superblock;
mod xattrs;

#[macro_export]
macro_rules! round {
    (UP, $x: expr, $y: expr) => {
        ($x + $y - 1) / $y * $y
    };
    (DOWN, $x: expr, $y: expr) => {
        ($x / $y) * $y
    };
}
