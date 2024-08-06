// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

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

pub(crate) trait Source {
    fn fill(&self, data: &mut [u8], offset: Off) -> SourceResult<u64>;
    fn get_temp_buffer(&self, offset: Off, maxsize: Off) -> SourceResult<TempBuffer> {
        let mut block: Page = EROFS_PAGE;
        let pa = PageAddress::from(offset);
        self.fill(&mut block, pa.page)
            .map(|sz| TempBuffer::new(block, pa.pg_off as usize, sz.min(maxsize) as usize))
    }
}

pub(crate) trait FileSource: Source {}

// This only allocates with in a
pub(crate) trait PageSource<'a>: Source {
    fn as_buf(&'a self, offset: Off, len: Off) -> SourceResult<RefBuffer<'a>>;
    fn as_buf_mut(&'a mut self, offset: Off, len: Off) -> SourceResult<RefBufferMut<'a>>;
}

pub(crate) trait Backend {
    fn fill(&self, data: &mut [u8], offset: Off) -> BackendResult<u64>;
    fn get_temp_buffer(&self, offset: Off, maxsize: Off) -> BackendResult<TempBuffer>;
}

pub(crate) trait FileBackend: Backend {}

pub(crate) trait MemoryBackend<'a>: Backend {
    fn as_buf(&'a self, offset: Off, len: Off) -> BackendResult<RefBuffer<'a>>;
    fn as_buf_mut(&'a mut self, offset: Off, len: Off) -> BackendResult<RefBufferMut<'a>>;
}

pub(crate) struct TempBuffer {
    block: Page,
    start: usize,
    maxsize: usize,
}

pub(crate) trait Buffer {
    fn content(&self) -> &[u8];
    fn iter_dir(&self) -> DirCollection<'_> {
        DirCollection::new(self.content())
    }
}

pub(crate) trait BufferMut: Buffer {
    fn content_mut(&mut self) -> &mut [u8];
}

impl TempBuffer {
    pub(crate) fn new(block: Page, start: usize, maxsize: usize) -> Self {
        Self {
            block,
            start,
            maxsize,
        }
    }
    pub(crate) const fn empty() -> Self {
        Self {
            block: EROFS_PAGE,
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

pub(crate) struct RefBuffer<'a> {
    buf: &'a [u8],
    start: usize,
    len: usize,
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
    pub(crate) fn new(buf: &'a [u8], start: usize, len: usize) -> Self {
        Self { buf, start, len }
    }
}

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
            let mut m = self.sbi.map(self.inode, self.offset);
            let pa = PageAddress::from(m.physical.start);
            let len = m.physical.len.min(pa.pg_len);
            m.physical.len = len;
            m.logical.len = len;
            self.offset += len;
            Some(m)
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
                if m.logical.len < EROFS_PAGE_SZ as Off {
                    let mut block = EROFS_PAGE;
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
                .as_buf(m.physical.start, m.physical.len.min(EROFS_PAGE_SZ))
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

pub(crate) trait ContinousBufferIter<'a>: Iterator<Item = Box<dyn Buffer + 'a>> {
    fn advance_off(&mut self, offset: Off);
}

impl<'a, B> ContinousBufferIter<'a> for ContinuousTempBufferIter<'a, B>
where
    B: FileBackend,
{
    fn advance_off(&mut self, offset: Off) {
        self.offset += offset;
        self.len -= offset;
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

        let pa = PageAddress::from(self.offset);
        let len = pa.pg_len.min(self.len);
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
}

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
                .get_temp_buffer(self.offset, EROFS_PAGE_SZ)
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

pub(crate) struct SkippableContinousIter<'a> {
    iter: Box<dyn ContinousBufferIter<'a> + 'a>,
    data: Box<dyn Buffer + 'a>,
    d_off: Off,
}

impl<'a> SkippableContinousIter<'a> {
    pub(crate) fn new(mut iter: Box<dyn ContinousBufferIter<'a> + 'a>) -> Self {
        let data = iter.next().unwrap();
        Self {
            iter,
            data,
            d_off: 0,
        }
    }
    pub(crate) fn skip(&mut self, offset: Off) {
        let d_len = self.data.content().len() as Off - self.d_off;

        if offset < d_len {
            self.d_off += offset;
        } else {
            self.d_off = 0;
            self.iter.advance_off(d_len);
            self.data = self.iter.next().unwrap();
        }
    }

    pub(crate) fn read(&mut self, buf: &mut [u8]) {
        let mut d_len = self.data.content().len() as Off - self.d_off;
        let mut b_off = 0 as Off;
        let b_len = buf.len() as Off;
        if d_len != 0 && d_len >= b_len {
            buf.clone_from_slice(
                &self.data.content()[self.d_off as usize..(self.d_off + b_len) as usize],
            );
            self.d_off += b_len;
        } else {
            buf[b_off as usize..(b_off + d_len) as usize]
                .copy_from_slice(&self.data.content()[self.d_off as usize..]);
            b_off += d_len;
            while b_off < b_len {
                self.d_off = 0;
                self.data = self.iter.next().unwrap();
                d_len = self.data.content().len() as Off;
                if d_len >= b_len - b_off {
                    buf[b_off as usize..]
                        .copy_from_slice(&self.data.content()[..(b_len - b_off) as usize]);
                    self.d_off = b_len - b_off;
                    return;
                } else {
                    buf[b_off as usize..(b_off + d_len) as usize]
                        .copy_from_slice(self.data.content());
                    b_off += d_len;
                }
            }
        }
    }

    pub(crate) fn try_cmp(&mut self, buf: &[u8]) -> Result<(), u64> {
        let d_len = self.data.content().len() as Off - self.d_off;
        let b_len = buf.len() as Off;
        let mut b_off = 0 as Off;

        if d_len != 0 && d_len >= b_len {
            if self.data.content()[self.d_off as usize..(self.d_off + b_len) as usize]
                == buf[0..b_len as usize]
            {
                Ok(())
            } else {
                Err(b_len)
            }
        } else {
            if d_len != 0 {
                let cmp_len = d_len.min(b_len);
                b_off += cmp_len;
                if self.data.content()[self.d_off as usize..(self.d_off + cmp_len) as usize]
                    != buf[0..cmp_len as usize]
                {
                    return Err(b_off);
                }
            }
            while b_off < b_len {
                self.d_off = 0;
                self.data = self.iter.next().unwrap();
                let d_len = self.data.content().len() as Off;
                let cmp_len = d_len.min(b_len - b_off);
                b_off += cmp_len;
                if self.data.content()[0..cmp_len as usize] != buf[b_off as usize..] {
                    return Err(b_off);
                }
            }
            Ok(())
        }
    }
}
