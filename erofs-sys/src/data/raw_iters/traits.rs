// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

use super::super::*;

/// Represents a basic iterator over a range of bytes from data backends.
/// The access order is guided by the block maps from the filesystem.
pub(crate) trait BufferMapIter<'a>: Iterator<Item = PosixResult<RefBuffer<'a>>> {}

/// Represents a basic iterator over a range of bytes from data backends.
/// Note that this is skippable and can be used to move the iterator's cursor forward.
pub(crate) trait ContinousBufferIter<'a>:
    Iterator<Item = PosixResult<RefBuffer<'a>>>
{
    fn advance_off(&mut self, offset: Off);
    fn eof(&self) -> bool;
}
