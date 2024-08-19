// Copyright 2024 Yiyang Wu SPDX-License-Identifier: MIT or GPL-2.0-later
pub(crate) mod uncompressed;

use alloc::boxed::Box;
use alloc::vec::Vec;

use super::alloc_helper::*;
use super::dir::*;
use super::inode::*;
use super::map::*;
use super::superblock::*;
use super::*;

use crate::round;

#[derive(Debug)]
pub(crate) enum SourceError {
    Dummy,
    OutBound,
}

#[derive(Debug)]
pub(crate) enum BackendError {
    Dummy,
}

pub(crate) type SourceResult<T> = Result<T, SourceError>;
pub(crate) type BackendResult<T> = Result<T, BackendError>;

/// Represent some sort of generic data source. This cound be file, memory or even network.
/// Note that users should never use this directly please use backends instead.
pub(crate) trait Source {
    fn fill(&self, data: &mut [u8], offset: Off) -> SourceResult<u64>;
    fn get_temp_buffer(&self, offset: Off, maxsize: Off) -> SourceResult<TempBuffer> {
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
    fn as_buf(&'a self, offset: Off, len: Off) -> SourceResult<RefBuffer<'a>>;
    fn as_buf_mut(&'a mut self, offset: Off, len: Off) -> SourceResult<RefBufferMut<'a>>;
}

/// Represents a generic data access backend that is backed by some sort of data source.
/// This often has temporary buffers to decompress the data from the data source.
/// The method signatures are the same as those of the Source trait.
pub(crate) trait Backend {
    fn fill(&self, data: &mut [u8], offset: Off) -> BackendResult<u64>;
    fn get_temp_buffer(&self, offset: Off, maxsize: Off) -> BackendResult<TempBuffer>;
}

/// Represents a file backend whose source is a file.
pub(crate) trait FileBackend: Backend {}

/// Represents a memory backend whose source is memory.
pub(crate) trait MemoryBackend<'a>: Backend {
    fn as_buf(&'a self, offset: Off, len: Off) -> BackendResult<RefBuffer<'a>>;
    fn as_buf_mut(&'a mut self, offset: Off, len: Off) -> BackendResult<RefBufferMut<'a>>;
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

/// Iterates over the data map represented by an inode.
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
    pub(crate) fn new(sbi: &'a FS, inode: &'b I, offset: Off) -> Self {
        Self {
            sbi,
            inode,
            offset,
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
            let result = self.sbi.map(self.inode, self.offset);
            match result {
                Ok(mut m) => {
                    let ba = DiskBlockAccessor::new(self.sbi.superblock(), m.physical.start);
                    let len = m.physical.len.min(ba.len);
                    m.physical.len = len;
                    m.logical.len = len;
                    self.offset += len;
                    Some(m)
                }
                Err(_) => None,
            }
        }
    }
}

pub(crate) trait BufferMapIter<'a>: Iterator<Item = Box<dyn Buffer + 'a>> {}

pub(crate) struct TempBufferMapIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: FileBackend,
    I: Inode,
{
    backend: &'a B,
    map_iter: MapIter<'a, 'b, FS, I>,
}

impl<'a, 'b, FS, B, I> TempBufferMapIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: FileBackend,
    I: Inode,
{
    pub(crate) fn new(backend: &'a B, map_iter: MapIter<'a, 'b, FS, I>) -> Self {
        Self { backend, map_iter }
    }
}

impl<'a, 'b, FS, B, I> Iterator for TempBufferMapIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: FileBackend,
    I: Inode,
{
    type Item = Box<dyn Buffer + 'a>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.map_iter.next() {
            Some(m) => {
                if m.logical.len < EROFS_TEMP_BLOCK_SZ as Off {
                    let mut block = EROFS_TEMP_BLOCK;
                    match self
                        .backend
                        .fill(&mut block[0..m.physical.len as usize], m.physical.start)
                    {
                        Ok(rlen) => Some(heap_alloc(TempBuffer::new(block, 0, rlen as usize))),
                        Err(_) => None,
                    }
                } else {
                    match self
                        .backend
                        .get_temp_buffer(m.physical.start, m.logical.len)
                    {
                        Ok(buffer) => Some(heap_alloc(buffer)),
                        Err(_) => None,
                    }
                }
            }
            None => None,
        }
    }
}

impl<'a, 'b, FS, B, I> BufferMapIter<'a> for TempBufferMapIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: FileBackend,
    I: Inode,
{
}

pub(crate) struct RefMapIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: MemoryBackend<'a>,
    I: Inode,
{
    backend: &'a B,
    map_iter: MapIter<'a, 'b, FS, I>,
}

impl<'a, 'b, FS, B, I> RefMapIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: MemoryBackend<'a>,
    I: Inode,
{
    pub(crate) fn new(backend: &'a B, map_iter: MapIter<'a, 'b, FS, I>) -> Self {
        Self { backend, map_iter }
    }
}

impl<'a, 'b, FS, B, I> Iterator for RefMapIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: MemoryBackend<'a>,
    I: Inode,
{
    type Item = Box<dyn Buffer + 'a>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.map_iter.next() {
            Some(m) => match self
                .backend
                .as_buf(m.physical.start, m.physical.len.min(EROFS_TEMP_BLOCK_SZ))
            {
                Ok(buf) => Some(heap_alloc(buf)),
                Err(_) => None,
            },
            None => None,
        }
    }
}

impl<'a, 'b, FS, B, I> BufferMapIter<'a> for RefMapIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: MemoryBackend<'a>,
    I: Inode,
{
}

pub(crate) struct ContinuousTempBufferIter<'a, B>
where
    B: FileBackend,
{
    backend: &'a B,
    offset: Off,
    len: Off,
}

impl<'a, B> ContinuousTempBufferIter<'a, B>
where
    B: FileBackend,
{
    pub(crate) fn new(backend: &'a B, offset: Off, len: Off) -> Self {
        Self {
            backend,
            offset,
            len,
        }
    }
}

impl<'a, B> Iterator for ContinuousTempBufferIter<'a, B>
where
    B: FileBackend,
{
    type Item = Box<dyn Buffer + 'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }

        let result: Option<Self::Item> = self
            .backend
            .get_temp_buffer(self.offset, self.len)
            .map_or_else(
                |_| None,
                |buffer| {
                    self.offset += buffer.content().len() as Off;
                    self.len -= buffer.content().len() as Off;
                    Some(heap_alloc(buffer) as Box<dyn Buffer + 'a>)
                },
            );
        result
    }
}

pub(crate) struct ContinuousRefIter<'a, B>
where
    B: MemoryBackend<'a>,
{
    backend: &'a B,
    offset: Off,
    len: Off,
    first: bool,
}

impl<'a, B> ContinuousRefIter<'a, B>
where
    B: MemoryBackend<'a>,
{
    pub(crate) fn new(backend: &'a B, offset: Off, len: Off) -> Self {
        Self {
            backend,
            offset,
            len,
            first: true,
        }
    }
}

/// Represents a basic iterator over a range of bytes from data backends.
/// Note that this is skippable and can be used to move the iterator's cursor forward.
pub(crate) trait ContinousBufferIter<'a>: Iterator<Item = Box<dyn Buffer + 'a>> {
    fn advance_off(&mut self, offset: Off);
    fn eof(&self) -> bool;
}

impl<'a, B> ContinousBufferIter<'a> for ContinuousTempBufferIter<'a, B>
where
    B: FileBackend,
{
    fn advance_off(&mut self, offset: Off) {
        self.offset += offset;
        self.len -= offset;
    }
    fn eof(&self) -> bool {
        self.len == 0
    }
}

impl<'a, B> Iterator for ContinuousRefIter<'a, B>
where
    B: MemoryBackend<'a>,
{
    type Item = Box<dyn Buffer + 'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }

        let pa = TempBlockAccessor::from(self.offset);
        let len = pa.len.min(self.len);
        let result: Option<Self::Item> = self.backend.as_buf(self.offset, len).map_or_else(
            |_| None,
            |x| {
                self.offset += x.content().len() as Off;
                self.len -= x.content().len() as Off;
                Some(heap_alloc(x) as Box<dyn Buffer + 'a>)
            },
        );
        result
    }
}

impl<'a, B> ContinousBufferIter<'a> for ContinuousRefIter<'a, B>
where
    B: MemoryBackend<'a>,
{
    fn advance_off(&mut self, offset: Off) {
        self.offset += offset;
        self.len -= offset
    }
    fn eof(&self) -> bool {
        self.len == 0
    }
}

/// This is used as a iterator to read the metadata buffer. The metadata buffer is a continous 4
/// bytes aligned collection of integers. This is used primarily when reading an inode's xattrs
/// indexe.
pub(crate) struct MetadataBufferIter<'a> {
    backend: &'a dyn Backend,
    buffer: TempBuffer,
    offset: Off,
    total: usize,
}

impl<'a> MetadataBufferIter<'a> {
    pub(crate) fn new(backend: &'a dyn Backend, offset: Off, total: usize) -> Self {
        Self {
            backend,
            buffer: TempBuffer::empty(),
            offset,
            total,
        }
    }
}

impl<'a> Iterator for MetadataBufferIter<'a> {
    type Item = Vec<u8>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.total == 0 {
            return None;
        }

        if self.buffer.start == self.buffer.maxsize {
            self.buffer = self
                .backend
                .get_temp_buffer(self.offset, EROFS_TEMP_BLOCK_SZ)
                .unwrap();
            self.offset += self.buffer.maxsize as Off;
        }

        let data = self.buffer.content();
        let size = u16::from_le_bytes([data[0], data[1]]) as usize;
        let mut result: Vec<u8> = Vec::new();
        extend_from_slice(&mut result, &data[2..size + 2]);
        self.buffer.start = round!(UP, self.buffer.start + size + 2, 4);
        self.total -= 1;
        Some(result)
    }
}

/// Represents a skippable continuous buffer iterator. This is used primarily for reading the
/// extended attributes. Since the key-value is flattened out in its original format.
pub(crate) struct SkippableContinousIter<'a> {
    iter: Box<dyn ContinousBufferIter<'a> + 'a>,
    data: Box<dyn Buffer + 'a>,
    cur: Off,
}

fn cmp_with_cursor_move(
    lhs: &[u8],
    rhs: &[u8],
    lhs_cur: &mut Off,
    rhs_cur: &mut Off,
    len: Off,
) -> bool {
    let result = lhs[*lhs_cur as usize..(*lhs_cur + len) as usize]
        == rhs[*rhs_cur as usize..(*rhs_cur + len) as usize];
    *lhs_cur += len;
    *rhs_cur += len;
    result
}

impl<'a> SkippableContinousIter<'a> {
    pub(crate) fn new(mut iter: Box<dyn ContinousBufferIter<'a> + 'a>) -> Self {
        let data = iter.next().unwrap();
        Self { iter, data, cur: 0 }
    }
    pub(crate) fn skip(&mut self, offset: Off) {
        let dlen = self.data.content().len() as Off - self.cur;

        if offset <= dlen {
            self.cur += offset;
        } else {
            self.cur = 0;
            self.iter.advance_off(dlen);
            self.data = self.iter.next().unwrap();
        }
    }

    pub(crate) fn read(&mut self, buf: &mut [u8]) {
        let mut dlen = self.data.content().len() as Off - self.cur;
        let mut bcur = 0 as Off;
        let blen = buf.len() as Off;
        if dlen != 0 && dlen >= blen {
            buf.clone_from_slice(
                &self.data.content()[self.cur as usize..(self.cur + blen) as usize],
            );
            self.cur += blen;
        } else {
            buf[bcur as usize..(bcur + dlen) as usize]
                .copy_from_slice(&self.data.content()[self.cur as usize..]);
            bcur += dlen;
            while bcur < blen {
                self.cur = 0;
                self.data = self.iter.next().unwrap();
                dlen = self.data.content().len() as Off;
                if dlen >= blen - bcur {
                    buf[bcur as usize..]
                        .copy_from_slice(&self.data.content()[..(blen - bcur) as usize]);
                    self.cur = blen - bcur;
                    return;
                } else {
                    buf[bcur as usize..(bcur + dlen) as usize].copy_from_slice(self.data.content());
                    bcur += dlen;
                }
            }
        }
    }

    pub(crate) fn try_cmp(&mut self, buf: &[u8]) -> Result<(), u64> {
        let dlen = self.data.content().len() as Off - self.cur;
        let blen = buf.len() as Off;
        let mut bcur = 0 as Off;

        if dlen != 0 && dlen >= blen {
            if cmp_with_cursor_move(self.data.content(), buf, &mut self.cur, &mut bcur, blen) {
                Ok(())
            } else {
                Err(bcur)
            }
        } else {
            if dlen != 0 {
                let clen = dlen.min(blen);
                if !cmp_with_cursor_move(self.data.content(), buf, &mut self.cur, &mut bcur, clen) {
                    return Err(bcur);
                }
            }
            while bcur < blen {
                self.cur = 0;
                self.data = self.iter.next().unwrap();
                let dlen = self.data.content().len() as Off;
                let clen = dlen.min(blen - bcur);
                if !cmp_with_cursor_move(self.data.content(), buf, &mut self.cur, &mut bcur, clen) {
                    return Err(bcur);
                }
            }

            Ok(())
        }
    }
    pub(crate) fn eof(&self) -> bool {
        self.data.content().len() as Off - self.cur == 0 && self.iter.eof()
    }
}
