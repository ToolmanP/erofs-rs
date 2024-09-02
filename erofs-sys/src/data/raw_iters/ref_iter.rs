// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

use super::super::*;
use super::*;
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
    type Item = PosixResult<Box<dyn Buffer + 'a>>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.map_iter.next() {
            Some(map) => match map {
                Ok(m) => {
                    match self
                        .backend
                        .as_buf(m.physical.start, m.physical.len.min(EROFS_TEMP_BLOCK_SZ))
                    {
                        Ok(buf) => Some(heap_alloc(buf).map(|v| v as Box<dyn Buffer + 'a>)),
                        Err(e) => Some(Err(e)),
                    }
                }
                Err(e) => Some(Err(e)),
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

impl<'a, B> Iterator for ContinuousRefIter<'a, B>
where
    B: MemoryBackend<'a>,
{
    type Item = PosixResult<Box<dyn Buffer + 'a>>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }

        let pa = TempBlockAccessor::from(self.offset);
        let len = pa.len.min(self.len);
        let result: Option<Self::Item> = self.backend.as_buf(self.offset, len).map_or_else(
            |e| Some(Err(e)),
            |x| {
                self.offset += x.content().len() as Off;
                self.len -= x.content().len() as Off;
                Some(heap_alloc(x).map(|v| v as Box<dyn Buffer + 'a>))
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
