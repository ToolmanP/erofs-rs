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
    type Item = PosixResult<Box<dyn Buffer + 'a>>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.map_iter.next() {
            Some(m) => {
                if m.logical.len < EROFS_TEMP_BLOCK_SZ as Off {
                    let mut block = EROFS_TEMP_BLOCK;
                    match self
                        .backend
                        .fill(&mut block[0..m.physical.len as usize], m.physical.start)
                    {
                        Ok(rlen) => Some(
                            heap_alloc(TempBuffer::new(block, 0, rlen as usize))
                                .map(|v| v as Box<dyn Buffer + 'a>),
                        ),
                        Err(e) => Some(Err(e)),
                    }
                } else {
                    match self
                        .backend
                        .get_temp_buffer(m.physical.start, m.logical.len)
                    {
                        Ok(buffer) => Some(heap_alloc(buffer).map(|v| v as Box<dyn Buffer + 'a>)),
                        Err(e) => Some(Err(e)),
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
    type Item = PosixResult<Box<dyn Buffer + 'a>>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }

        let result: Option<Self::Item> = self
            .backend
            .get_temp_buffer(self.offset, self.len)
            .map_or_else(
                |e| Some(Err(e)),
                |buffer| {
                    self.offset += buffer.content().len() as Off;
                    self.len -= buffer.content().len() as Off;
                    Some(heap_alloc(buffer).map(|v| v as Box<dyn Buffer + 'a>))
                },
            );
        result
    }
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
