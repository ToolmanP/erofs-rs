// Copyright 2024 Yiyang Wu SPDX-License-Identifier: MIT or GPL-2.0-or-later
pub(crate) mod backends;
pub(crate) mod raw_iters;

use alloc::boxed::Box;
use alloc::vec::Vec;

use super::alloc_helper::*;
use super::dir::*;
use super::inode::*;
use super::map::*;
use super::superblock::*;
use super::*;

/// Represent some sort of generic data source. This cound be file, memory or even network.
/// Note that users should never use this directly please use backends instead.
pub(crate) trait Source {
    fn fill(&self, data: &mut [u8], offset: Off) -> PosixResult<u64>;
}

/// Represents a file source.
pub(crate) trait FileSource: Source {}

// Represents a memory source. Note that as_buf and as_buf_mut should only represent memory within
// a page. Cross page memory is not supported and treated as an error.
pub(crate) trait PageSource<'a>: Source {
    fn as_buf(&'a self, offset: Off, len: Off) -> PosixResult<RefBuffer<'a>>;
}

/// Represents a generic data access backend that is backed by some sort of data source.
/// This often has temporary buffers to decompress the data from the data source.
/// The method signatures are the same as those of the Source trait.
pub(crate) trait Backend {
    fn fill(&self, data: &mut [u8], offset: Off) -> PosixResult<u64>;
}

/// Represents a file backend whose source is a file.
pub(crate) trait FileBackend: Backend {}

/// Represents a memory backend whose source is memory.
pub(crate) trait MemoryBackend<'a>: Backend {
    fn as_buf(&'a self, offset: Off, len: Off) -> PosixResult<RefBuffer<'a>>;
}

/// Represents a TempBuffer which owns a temporary on-stack/on-heap buffer.
/// Note that file or network backend can only use this since they can't access the data from the
/// memory directly.
pub(crate) struct TempBuffer {
    pub(crate) block: Vec<u8>,
    pub(crate) start: usize,
    pub(crate) maxsize: usize,
}

/// Represents a buffer trait which can yield its internal reference or be casted as an iterator of
/// DirEntries.
pub(crate) trait Buffer {
    fn content(&self) -> &[u8];
    fn iter_dir(&self) -> DirCollection<'_> {
        DirCollection::new(self.content())
    }
}

impl TempBuffer {
    pub(crate) fn new(block: Vec<u8>, start: usize, maxsize: usize) -> Self {
        Self {
            block,
            start,
            maxsize,
        }
    }
}

impl Buffer for TempBuffer {
    fn content(&self) -> &[u8] {
        &self.block[self.start..self.start + self.maxsize]
    }
}

/// Represents a buffer that holds a reference to a slice of data that
/// is borrowed from the thin air.
pub(crate) struct RefBuffer<'a> {
    buf: &'a [u8],
    start: usize,
    len: usize,
    put_buf: fn(*mut core::ffi::c_void),
}

impl<'a> Buffer for RefBuffer<'a> {
    fn content(&self) -> &[u8] {
        &self.buf[self.start..self.start + self.len]
    }
}

impl<'a> RefBuffer<'a> {
    pub(crate) fn new(
        buf: &'a [u8],
        start: usize,
        len: usize,
        put_buf: fn(*mut core::ffi::c_void),
    ) -> Self {
        Self {
            buf,
            start,
            len,
            put_buf,
        }
    }
}

impl<'a> Drop for RefBuffer<'a> {
    fn drop(&mut self) {
        (self.put_buf)(self.buf.as_ptr() as *mut core::ffi::c_void)
    }
}
