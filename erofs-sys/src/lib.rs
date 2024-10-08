#![no_std]
#![allow(dead_code)]
// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-or-later

//! A pure Rust implementation of the EROFS filesystem.
//! Technical Details are documented in the [EROFS Documentation](https://erofs.docs.kernel.org/en/latest/)

// It's unavoidable to import alloc here. Since there are so many backends there and if we want to
// to use trait object to export Filesystem pointer. The alloc crate here is necessary.

#[cfg(not(CONFIG_EROFS_FS = "y"))]
extern crate alloc;

/// Erofs requires block index to a 32 bit unsigned integer.
pub type Blk = u32;
/// Erofs requires normal offset to be a 64bit unsigned integer.
pub type Off = u64;
/// Erofs requires inode nid to be a 64bit unsigned integer.
pub type Nid = u64;

pub(crate) const EROFS_SUPER_OFFSET: Off = 1024;

pub(crate) mod alloc_helper;
pub(crate) mod compression;
/// Data Module
pub mod data;
pub(crate) mod devices;
/// Dir Module
pub mod dir;
/// Errno Module
pub mod errnos;
/// Inode Module
pub mod inode;
pub(crate) mod map;
/// Operations Modules
pub mod operations;
/// Superblock
pub mod superblock;
/// Xattrs Module
pub mod xattrs;
pub(crate) use errnos::Errno;
/// Documentation
pub type PosixResult<T> = Result<T, errnos::Errno>;

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

pub use superblock::{file, mem};
