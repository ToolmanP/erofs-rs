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
    sb: &'a SuperBlock,
    backend: &'a B,
    map_iter: MapIter<'a, 'b, FS, I>,
}

impl<'a, 'b, FS, B, I> RefMapIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: MemoryBackend<'a>,
    I: Inode,
{
    pub(crate) fn new(
        sb: &'a SuperBlock,
        backend: &'a B,
        map_iter: MapIter<'a, 'b, FS, I>,
    ) -> Self {
        Self {
            sb,
            backend,
            map_iter,
        }
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
                    let accessor = self.sb.blk_access(m.physical.start);
                    let len = m.physical.len.min(accessor.len);
                    match self.backend.as_buf(m.physical.start, len) {
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
    sb: &'a SuperBlock,
    backend: &'a B,
    offset: Off,
    len: Off,
}

impl<'a, B> ContinuousRefIter<'a, B>
where
    B: MemoryBackend<'a>,
{
    pub(crate) fn new(sb: &'a SuperBlock, backend: &'a B, offset: Off, len: Off) -> Self {
        Self {
            sb,
            backend,
            offset,
            len,
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
        let accessor = self.sb.blk_access(self.offset);
        let len = accessor.len.min(self.len);
        let result: Option<Self::Item> = self.backend.as_buf(self.offset, len).map_or_else(
            |e| Some(Err(e)),
            |buf| {
                self.offset += len;
                self.len -= len;
                Some(heap_alloc(buf).map(|v| v as Box<dyn Buffer + 'a>))
            },
        );
        result
    }
}

impl<'a, B> ContinuousBufferIter<'a> for ContinuousRefIter<'a, B>
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
