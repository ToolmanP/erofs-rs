#![no_std]
// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

pub(crate) const EROFS_BLOCK_SZ: usize = 4096;
pub(crate) const EROFS_EMPTY_BLOCK: Block = [0; EROFS_BLOCK_SZ];
pub(crate) const EROFS_SUPER_OFFSET: Off = 1024;

// It's unavoidable to import alloc here. Since there are so many backends there and if we want to
// to use trait object to export Filesystem pointer. The alloc crate here is necessary.
extern crate alloc;

pub type Block = [u8; EROFS_BLOCK_SZ];
pub type Blk = u32;
pub type Off = u64;
pub type Nid = u64;

mod compression;
mod data;
mod dir;
mod inode;
mod map;
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
