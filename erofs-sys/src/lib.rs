#![no_std]
#![allow(dead_code)]
// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

//! A pure Rust implementation of the EROFS filesystem.
//! Technical Details are documented in the [EROFS Documentation](https://erofs.docs.kernel.org/en/latest/)

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

/// Erofs Operates on the block/page size of 4096 we respect that.
pub type Block = [u8; EROFS_BLOCK_SZ as usize];
/// Erofs requires block index to a 32 bit unsigned integer.
pub type Blk = u32;
/// Erofs requires normal offset to be a 64bit unsigned integer.
pub type Off = u64;
/// Erofs requires inode nid to be a 64bit unsigned integer.
pub type Nid = u64;

pub(crate) mod alloc_helper;
pub(crate) mod compression;
pub(crate) mod data;
pub(crate) mod devices;
pub(crate) mod dir;
pub(crate) mod inode;
pub(crate) mod map;
pub(crate) mod operations;
pub(crate) mod superblock;
pub(crate) mod xattrs;

/// Helper macro to round up or down a number.
#[macro_export]
macro_rules! round {
    (UP, $x: expr, $y: expr) => {
        ($x + $y - 1) / $y * $y
    };
    (DOWN, $x: expr, $y: expr) => {
        ($x / $y) * $y
    };
}
