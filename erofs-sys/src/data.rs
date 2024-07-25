// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

pub mod uncompressed;

use alloc::boxed::Box;

use super::inode::*;
use super::map::*;
use super::superblock::FileSystem;
use super::*;

#[derive(Debug)]
pub(crate) enum SourceError {
    Dummy,
}

#[derive(Debug)]
pub(crate) enum BackendError {
    Dummy,
}

pub(crate) type SourceResult<T> = Result<T, SourceError>;
pub(crate) type BackendResult<T> = Result<T, BackendError>;

pub(crate) trait Source {
    fn fill(&self, data: &mut [u8], offset: Off) -> SourceResult<()>;
    fn get_block(&self, offset: Off) -> SourceResult<Block> {
        let mut block: Block = EROFS_EMPTY_BLOCK;
        self.fill(&mut block, round!(DOWN, offset, EROFS_BLOCK_SZ as Off))
            .map(|()| block)
    }
}

pub(crate) trait FileSource: Source {}

pub(crate) trait MemorySource<'a>: Source {
    fn as_buf(&'a self, offset: Off, len: Off) -> SourceResult<MemBuffer<'a>>;
    fn as_buf_mut(&'a mut self, offset: Off, len: Off) -> SourceResult<MemBufferMut<'a>>;
}

pub(crate) trait Backend {
    fn fill(&self, data: &mut [u8], offset: Off) -> BackendResult<()>;
    fn get_block(&self, offset: Off) -> BackendResult<Block>;
}

pub(crate) trait FileBackend: Backend {}

pub(crate) trait MemoryBackend<'a>: Backend {
    fn as_buf(&'a self, offset: Off, len: Off) -> BackendResult<MemBuffer<'a>>;
    fn as_buf_mut(&'a mut self, offset: Off, len: Off) -> BackendResult<MemBufferMut<'a>>;
}

pub(crate) struct TempBuffer {
    block: Block,
    maxsize: usize,
}

pub(crate) trait Buffer {
    fn content(&self) -> &[u8];
}

pub(crate) trait BufferMut: Buffer {
    fn content_mut(&mut self) -> &mut [u8];
}

impl TempBuffer {
    pub(crate) fn new(block: Block, maxsize: usize) -> Self {
        Self { block, maxsize }
    }
}

impl Buffer for TempBuffer {
    fn content(&self) -> &[u8] {
        &self.block[0..self.maxsize]
    }
}

impl BufferMut for TempBuffer {
    fn content_mut(&mut self) -> &mut [u8] {
        &mut self.block[0..self.maxsize]
    }
}

pub(crate) struct MemBuffer<'a> {
    buf: &'a [u8],
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

impl<'a> Buffer for MemBuffer<'a> {
    fn content(&self) -> &[u8] {
        self.buf
    }
}

impl<'a> MemBuffer<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }
}

pub(crate) struct MemBufferMut<'a> {
    buf: &'a mut [u8],
    put_buf: fn(*mut core::ffi::c_void),
}

impl<'a> MemBufferMut<'a> {
    pub fn new(buf: &'a mut [u8], put_buf: fn(*mut core::ffi::c_void)) -> Self {
        Self { buf, put_buf }
    }
}

impl<'a> Drop for MemBufferMut<'a> {
    fn drop(&mut self) {
        (self.put_buf)(self.buf.as_mut_ptr() as *mut core::ffi::c_void)
    }
}

pub(crate) struct MapIter<'a, 'b, FS, I>
where
    FS: FileSystem<I>,
    I: Inode,
{
    sbi: &'a FS,
    inode: &'b I,
    offset: Off,
    len: Off,
}

impl<'a, 'b, FS, I> MapIter<'a, 'b, FS, I>
where
    FS: FileSystem<I>,
    I: Inode,
{
    pub fn new(sbi: &'a FS, inode: &'b I) -> Self {
        Self {
            sbi,
            inode,
            offset: 0,
            len: inode.info().file_size(),
        }
    }
}

impl<'a, 'b, FS, I> Iterator for MapIter<'a, 'b, FS, I>
where
    FS: FileSystem<I>,
    I: Inode,
{
    type Item = Map;
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.len {
            None
        } else {
            let m = self.sbi.map(self.inode, self.offset);
            self.offset += m.logical.len.min(EROFS_BLOCK_SZ as u64);
            Some(m)
        }
    }
}

pub(crate) struct TempBufferIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: FileBackend,
    I: Inode,
{
    backend: &'a B,
    map_iter: MapIter<'a, 'b, FS, I>,
}

impl<'a, 'b, FS, B, I> TempBufferIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: FileBackend,
    I: Inode,
{
    pub(crate) fn new(backend: &'a B, map_iter: MapIter<'a, 'b, FS, I>) -> Self {
        Self { backend, map_iter }
    }
}

impl<'a, 'b, FS, B, I> Iterator for TempBufferIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: FileBackend,
    I: Inode,
{
    type Item = Box<dyn Buffer + 'a>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.map_iter.next() {
            Some(m) => {
                if m.logical.len < EROFS_BLOCK_SZ as Off {
                    let mut block = EROFS_EMPTY_BLOCK;
                    match self
                        .backend
                        .fill(&mut block[0..m.physical.len as usize], m.physical.start)
                    {
                        Ok(()) => Some(Box::new(TempBuffer::new(block, m.physical.len as usize))),
                        Err(_) => None,
                    }
                } else {
                    match self.backend.get_block(m.physical.start) {
                        Ok(block) => Some(Box::new(TempBuffer::new(block, EROFS_BLOCK_SZ))),
                        Err(_) => None,
                    }
                }
            }
            None => None,
        }
    }
}

pub(crate) struct RefIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: MemoryBackend<'a>,
    I: Inode,
{
    backend: &'a B,
    map_iter: MapIter<'a, 'b, FS, I>,
}

impl<'a, 'b, FS, B, I> RefIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: MemoryBackend<'a>,
    I: Inode,
{
    pub(crate) fn new(backend: &'a B, map_iter: MapIter<'a, 'b, FS, I>) -> Self {
        Self { backend, map_iter }
    }
}

impl<'a, 'b, FS, B, I> Iterator for RefIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: MemoryBackend<'a>,
    I: Inode,
{
    type Item = Box<dyn Buffer + 'a>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.map_iter.next() {
            Some(m) => match self.backend.as_buf(m.physical.start, m.physical.len) {
                Ok(buf) => Some(Box::new(buf)),
                Err(_) => None,
            },
            None => None,
        }
    }
}
