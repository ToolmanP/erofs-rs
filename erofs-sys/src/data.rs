// Copyright 2024 Yiyang Wu SPDX-License-Identifier: MIT or GPL-2.0-later
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

use crate::round;

/// Represent some sort of generic data source. This cound be file, memory or even network.
/// Note that users should never use this directly please use backends instead.
pub(crate) trait Source {
    fn fill(&self, data: &mut [u8], offset: Off) -> PosixResult<u64>;
    fn get_temp_buffer(&self, offset: Off, maxsize: Off) -> PosixResult<TempBuffer> {
        let mut block: TempBlock = EROFS_TEMP_BLOCK;
        let accessor = TempBlockAccessor::from(offset);
        self.fill(&mut block, accessor.base).map(|sz| {
            TempBuffer::new(
                block,
                accessor.off as usize,
                accessor.len.min(sz).min(maxsize) as usize,
            )
        })
    }
}

/// Represents a file source.
pub(crate) trait FileSource: Source {}

// Represents a memory source. Note that as_buf and as_buf_mut should only represent memory within
// a page. Cross page memory is not supported and treated as an error.
pub(crate) trait PageSource<'a>: Source {
    fn as_buf(&'a self, offset: Off, len: Off) -> PosixResult<RefBuffer<'a>>;
    fn as_buf_mut(&'a mut self, offset: Off, len: Off) -> PosixResult<RefBufferMut<'a>>;
}

/// Represents a generic data access backend that is backed by some sort of data source.
/// This often has temporary buffers to decompress the data from the data source.
/// The method signatures are the same as those of the Source trait.
pub(crate) trait Backend {
    fn fill(&self, data: &mut [u8], offset: Off) -> PosixResult<u64>;
    fn get_temp_buffer(&self, offset: Off, maxsize: Off) -> PosixResult<TempBuffer>;
}

/// Represents a file backend whose source is a file.
pub(crate) trait FileBackend: Backend {}

/// Represents a memory backend whose source is memory.
pub(crate) trait MemoryBackend<'a>: Backend {
    fn as_buf(&'a self, offset: Off, len: Off) -> PosixResult<RefBuffer<'a>>;
    fn as_buf_mut(&'a mut self, offset: Off, len: Off) -> PosixResult<RefBufferMut<'a>>;
}

/// Represents a TempBuffer which owns a temporary on-stack/on-heap buffer.
/// Note that file or network backend can only use this since they can't access the data from the
/// memory directly.
pub(crate) struct TempBuffer {
    block: TempBlock,
    start: usize,
    maxsize: usize,
}

/// Represents a buffer trait which can yield its internal reference or be casted as an iterator of
/// DirEntries.
pub(crate) trait Buffer {
    fn content(&self) -> &[u8];
    fn iter_dir(&self) -> DirCollection<'_> {
        DirCollection::new(self.content())
    }
}

/// Represents a mutable buffer trait which can yield its internal mutable reference.
pub(crate) trait BufferMut: Buffer {
    fn content_mut(&mut self) -> &mut [u8];
}

impl TempBuffer {
    pub(crate) fn new(block: TempBlock, start: usize, maxsize: usize) -> Self {
        Self {
            block,
            start,
            maxsize,
        }
    }
    pub(crate) const fn empty() -> Self {
        Self {
            block: EROFS_TEMP_BLOCK,
            start: 0,
            maxsize: 0,
        }
    }
}

impl Buffer for TempBuffer {
    fn content(&self) -> &[u8] {
        &self.block[self.start..self.start + self.maxsize]
    }
}

impl BufferMut for TempBuffer {
    fn content_mut(&mut self) -> &mut [u8] {
        &mut self.block[self.start..self.maxsize + self.start]
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

impl Buffer for [u8] {
    fn content(&self) -> &[u8] {
        self
    }
}

impl BufferMut for [u8] {
    fn content_mut(&mut self) -> &mut [u8] {
        self
    }
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

/// Represents a mutable buffer that holds a reference to a slice of data
/// that is borrowed from the thin air.
pub(crate) struct RefBufferMut<'a> {
    buf: &'a mut [u8],
    start: usize,
    len: usize,
    put_buf: fn(*mut core::ffi::c_void),
}

impl<'a> RefBufferMut<'a> {
    pub(crate) fn new(
        buf: &'a mut [u8],
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

impl<'a> Buffer for RefBufferMut<'a> {
    fn content(&self) -> &[u8] {
        &self.buf[self.start..self.start + self.len]
    }
}

impl<'a> BufferMut for RefBufferMut<'a> {
    fn content_mut(&mut self) -> &mut [u8] {
        &mut self.buf[self.start..self.start + self.len]
    }
}

impl<'a> Drop for RefBufferMut<'a> {
    fn drop(&mut self) {
        (self.put_buf)(self.buf.as_mut_ptr() as *mut core::ffi::c_void)
    }
}
