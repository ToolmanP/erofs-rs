// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

use super::super::*;
use super::traits::*;

pub(crate) struct TempBufferMapIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: FileBackend,
    I: Inode,
{
    sb: &'a SuperBlock,
    backend: &'a B,
    map_iter: MapIter<'a, 'b, FS, I>,
}

impl<'a, 'b, FS, B, I> TempBufferMapIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: FileBackend,
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
    fn try_yield(&mut self, map: Map) -> PosixResult<Box<dyn Buffer + 'a>> {
        let accessor = self.sb.blk_access(map.physical.start);
        let len = accessor.len.min(map.physical.len);
        let mut block = vec_with_capacity(len as usize).unwrap();
        self.backend.fill(&mut block, map.physical.start)?;
        heap_alloc(TempBuffer::new(block, 0, len as usize)).map(|v| v as Box<dyn Buffer + 'a>)
    }
}

impl<'a, 'b, FS, B, I> Iterator for TempBufferMapIter<'a, 'b, FS, B, I>
where
    FS: FileSystem<I>,
    B: FileBackend,
    I: Inode,
{
    type Item = PosixResult<Box<dyn Buffer + 'a>>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.map_iter.next() {
            Some(map) => match map {
                Ok(m) => Some(self.try_yield(m)),
                Err(e) => Some(Err(e)),
            },
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

pub(crate) struct ContinuousTempBufferIter<'a, B>
where
    B: FileBackend,
{
    sb: &'a SuperBlock,
    backend: &'a B,
    offset: Off,
    len: Off,
}

impl<'a, B> ContinuousTempBufferIter<'a, B>
where
    B: FileBackend,
{
    pub(crate) fn new(sb: &'a SuperBlock, backend: &'a B, offset: Off, len: Off) -> Self {
        Self {
            sb,
            backend,
            offset,
            len,
        }
    }
    fn try_yield(&mut self) -> PosixResult<Box<dyn Buffer + 'a>> {
        let accessor = self.sb.blk_access(self.offset);
        let len = self.len.min(accessor.len);
        let mut block = vec_with_capacity(len as usize)?;
        self.backend.fill(&mut block, self.offset)?;
        self.offset += len;
        self.len -= len;
        heap_alloc(TempBuffer::new(block, 0, len as usize)).map(|v| v as Box<dyn Buffer + 'a>)
    }
}

impl<'a, B> Iterator for ContinuousTempBufferIter<'a, B>
where
    B: FileBackend,
{
    type Item = PosixResult<Box<dyn Buffer + 'a>>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }
        Some(self.try_yield())
    }
}

impl<'a, B> ContinuousBufferIter<'a> for ContinuousTempBufferIter<'a, B>
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
