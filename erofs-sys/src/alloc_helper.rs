// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

/// This module provides helper functions for the alloc crate
/// Note that in linux kernel, the allocation is fallible however in userland it is not.
/// Since most of the functions depend on infallible allocation, here we provide helper functions
/// so that most of codes don't need to be changed.

#[cfg(CONFIG_EROFS_FS = "y")]
use kernel::prelude::*;

use alloc::boxed::Box;
use alloc::vec::Vec;

pub(crate) fn push_vec<T>(v: &mut Vec<T>, value: T) {
    match () {
        #[cfg(CONFIG_EROFS_FS = "y")]
        () => {
            v.push(value, GFP_KERNEL).unwrap();
        }
        #[cfg(not(CONFIG_EROFS_FS = "y"))]
        () => {
            v.push(value);
        }
    }
}

pub(crate) fn extend_from_slice<T: Clone>(v: &mut Vec<T>, slice: &[T]) {
    match () {
        #[cfg(CONFIG_EROFS_FS = "y")]
        () => {
            v.extend_from_slice(slice, GFP_KERNEL).unwrap();
        }
        #[cfg(not(CONFIG_EROFS_FS = "y"))]
        () => {
            v.extend_from_slice(slice);
        }
    }
}

pub(crate) fn heap_alloc<T>(value: T) -> Box<T> {
    match () {
        #[cfg(CONFIG_EROFS_FS = "y")]
        () => Box::new(value, GFP_KERNEL).unwrap(),
        #[cfg(not(CONFIG_EROFS_FS = "y"))]
        () => Box::new(value),
    }
}
