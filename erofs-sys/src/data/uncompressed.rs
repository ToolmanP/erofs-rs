// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use super::*;

pub(crate) struct UncompressedBackend<T>
where
    T: Source,
{
    source: T,
}

impl<T> Backend for UncompressedBackend<T>
where
    T: Source,
{
    fn fill(&self, data: &mut [u8], offset: Off) -> BackendResult<()> {
        self.source
            .fill(data, offset)
            .map_err(|_| BackendError::Dummy)
    }
    fn get_block(&self, offset: Off) -> BackendResult<Block> {
        match self.source.get_block(offset) {
            Ok(block) => Ok(block),
            Err(_) => Err(BackendError::Dummy),
        }
    }
}

impl<T> FileBackend for UncompressedBackend<T> where T: FileSource {}

impl<'a, T> MemoryBackend<'a> for UncompressedBackend<T>
where
    T: MemorySource<'a>,
{
    fn as_buf(&'a self, offset: Off, len: Off) -> BackendResult<MemBuffer<'a>> {
        self.source
            .as_buf(offset, len)
            .map_err(|_| BackendError::Dummy)
    }
    fn as_buf_mut(&'a mut self, offset: Off, len: Off) -> BackendResult<MemBufferMut<'a>> {
        self.source
            .as_buf_mut(offset, len)
            .map_err(|_| BackendError::Dummy)
    }
}


impl<T: Source> UncompressedBackend<T> {
    pub(crate) fn new(source: T) -> Self {
        Self { source }
    }
}

impl<T> From<T> for UncompressedBackend<T>
where
    T: Source,
{
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
