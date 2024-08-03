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
    fn fill(&self, data: &mut [u8], offset: Off) -> BackendResult<u64> {
        self.source
            .fill(data, offset)
            .map_or_else(|_| Err(BackendError::Dummy), Ok)
    }
    fn get_temp_buffer(&self, offset: Off, maxsize: Off) -> BackendResult<TempBuffer> {
        match self.source.get_temp_buffer(offset, maxsize) {
            Ok(buffer) => Ok(buffer),
            Err(_) => Err(BackendError::Dummy),
        }
    }
}

impl<T> FileBackend for UncompressedBackend<T> where T: Source {}

impl<'a, T> MemoryBackend<'a> for UncompressedBackend<T>
where
    T: PageSource<'a>,
{
    fn as_buf(&'a self, offset: Off, len: Off) -> BackendResult<RefBuffer<'a>> {
        self.source
            .as_buf(offset, len)
            .map_err(|_| BackendError::Dummy)
    }
    fn as_buf_mut(&'a mut self, offset: Off, len: Off) -> BackendResult<RefBufferMut<'a>> {
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
