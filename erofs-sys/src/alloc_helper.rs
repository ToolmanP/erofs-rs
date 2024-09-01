// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

/// This module provides helper functions for the alloc crate
/// Note that in linux kernel, the allocation is fallible however in userland it is not.
/// Since most of the functions depend on infallible allocation, here we provide helper functions
/// so that most of codes don't need to be changed.

#[cfg(CONFIG_EROFS_FS = "y")]
use kernel::prelude::*;

#[cfg(not(CONFIG_EROFS_FS = "y"))]
use alloc::vec;

use alloc::boxed::Box;
use alloc::vec::Vec;

use super::PosixResult;

pub(crate) fn push_vec<T>(v: &mut Vec<T>, value: T) -> PosixResult<()> {
    match () {
        #[cfg(CONFIG_EROFS_FS = "y")]
        () => v
            .push(value, GFP_KERNEL)
            .map_or_else(|_| Err(Errno::ENOMEM), |_| Ok(())),
        #[cfg(not(CONFIG_EROFS_FS = "y"))]
        () => {
            v.push(value);
            Ok(())
        }
    }
}

pub(crate) fn extend_from_slice<T: Clone>(v: &mut Vec<T>, slice: &[T]) -> PosixResult<()> {
    match () {
        #[cfg(CONFIG_EROFS_FS = "y")]
        () => v
            .extend_from_slice(slice, GFP_KERNEL)
            .map_or_else(|_| Err(Errno::ENOMEM), |_| Ok(())),
        #[cfg(not(CONFIG_EROFS_FS = "y"))]
        () => {
            v.extend_from_slice(slice);
            Ok(())
        }
    }
}

pub(crate) fn heap_alloc<T>(value: T) -> PosixResult<Box<T>> {
    match () {
        #[cfg(CONFIG_EROFS_FS = "y")]
        () => Box::new(value, GFP_KERNEL).map_or_else(|_| Err(Errno::ENOMEM), |v| Ok(v)),
        #[cfg(not(CONFIG_EROFS_FS = "y"))]
        () => Ok(Box::new(value)),
    }
}

pub(crate) fn vec_with_capacity<T: Default + Clone>(capacity: usize) -> PosixResult<Vec<T>> {
    match () {
        #[cfg(CONFIG_EROFS_FS = "y")]
        () => {
            Vec::with_capacity(capacity, GFP_KERNEL).map_or_else(|_| Err(Errno::ENOMEM), |v| Ok(v))
        }
        #[cfg(not(CONFIG_EROFS_FS = "y"))]
        () => Ok(vec![Default::default(); capacity]),
    }
}
