// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-or-later

use super::super::*;

/// An uncompressed Backend for Data Source
pub struct UncompressedBackend<T>
where
    T: Source,
{
    source: T,
}

impl<T> Backend for UncompressedBackend<T>
where
    T: Source,
{
    fn fill(&self, data: &mut [u8], device_id: i32, offset: Off) -> PosixResult<u64> {
        self.source.fill(data, device_id, offset)
    }
}
impl<T> FileBackend for UncompressedBackend<T> where T: Source {}

impl<'a, T> MemoryBackend<'a> for UncompressedBackend<T>
where
    T: PageSource<'a>,
{
    fn as_buf(&'a self, device_id: i32, offset: Off, len: Off) -> PosixResult<RefBuffer<'a>> {
        self.source.as_buf(device_id, offset, len)
    }
}

impl<T: Source> UncompressedBackend<T> {
    /// Create a new uncompressed backend from source.
    pub fn new(source: T) -> Self {
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
