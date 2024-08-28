#![no_std]
#![allow(dead_code)]
// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

//! A pure Rust implementation of the EROFS filesystem.
//! Technical Details are documented in the [EROFS Documentation](https://erofs.docs.kernel.org/en/latest/)

// It's unavoidable to import alloc here. Since there are so many backends there and if we want to
// to use trait object to export Filesystem pointer. The alloc crate here is necessary.

#[cfg(not(CONFIG_EROFS_FS = "y"))]
extern crate alloc;

/// Erofs requires block index to a 32 bit unsigned integer.
pub(crate) type Blk = u32;
/// Erofs requires normal offset to be a 64bit unsigned integer.
pub(crate) type Off = u64;
/// Erofs requires inode nid to be a 64bit unsigned integer.
pub(crate) type Nid = u64;

pub(crate) const EROFS_SUPER_OFFSET: Off = 1024;
pub(crate) const EROFS_TEMP_BLOCK: TempBlock = [0; EROFS_TEMP_BLOCK_SZ as usize];
pub(crate) const EROFS_TEMP_BLOCK_BITS: Off = 12;
pub(crate) const EROFS_TEMP_BLOCK_SZ: Off = 1 << EROFS_TEMP_BLOCK_BITS;
pub(crate) const EROFS_TEMP_BLOCK_MASK: Off = EROFS_TEMP_BLOCK_SZ - 1;

/// Erofs's maximum block is 4KB, so we use 4KB as the temp block size.
pub(crate) type TempBlock = [u8; EROFS_TEMP_BLOCK_SZ as usize];

/// Used for temp buffer address calculation
pub(crate) struct TempBlockAccessor {
    pub(crate) base: Off,
    pub(crate) off: Off,
    pub(crate) len: Off,
}

impl From<u64> for TempBlockAccessor {
    fn from(address: Off) -> Self {
        TempBlockAccessor {
            base: (address >> EROFS_TEMP_BLOCK_BITS) << EROFS_TEMP_BLOCK_BITS,
            off: address & EROFS_TEMP_BLOCK_MASK,
            len: EROFS_TEMP_BLOCK_SZ - (address & EROFS_TEMP_BLOCK_MASK),
        }
    }
}

pub(crate) mod alloc_helper;
pub(crate) mod compression;
pub(crate) mod data;
pub(crate) mod devices;
pub(crate) mod dir;
pub(crate) mod errnos;
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
