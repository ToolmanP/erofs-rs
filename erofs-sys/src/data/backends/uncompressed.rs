// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-or-later

use super::super::*;

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
    fn fill(&self, data: &mut [u8], offset: Off) -> PosixResult<u64> {
        self.source.fill(data, offset)
    }
}
impl<T> FileBackend for UncompressedBackend<T> where T: Source {}

impl<'a, T> MemoryBackend<'a> for UncompressedBackend<T>
where
    T: PageSource<'a>,
{
    fn as_buf(&'a self, offset: Off, len: Off) -> PosixResult<RefBuffer<'a>> {
        self.source.as_buf(offset, len)
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
