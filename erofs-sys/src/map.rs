// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

use super::inode::*;
use super::superblock::*;
use super::*;

pub(crate) const MAP_MAPPED: u32 = 0x0001;
pub(crate) const MAP_META: u32 = 0x0002;
pub(crate) const MAP_ENCODED: u32 = 0x0004;
pub(crate) const MAP_FULL_MAPPED: u32 = 0x0008;
pub(crate) const MAP_FRAGMENT: u32 = 0x0010;
pub(crate) const MAP_PARTIAL_REF: u32 = 0x0020;

#[derive(Debug, Default)]
#[repr(C)]
pub(crate) struct Segment {
    pub(crate) start: Off,
    pub(crate) len: Off,
}

#[derive(Debug, Default)]
#[repr(C)]
pub(crate) struct Map {
    pub(crate) logical: Segment,
    pub(crate) physical: Segment,
    pub(crate) device_id: u16,
    pub(crate) algorithm_format: u16,
    pub(crate) map_type: MapType,
}

#[derive(Debug, Default)]
pub(crate) enum MapType {
    Meta,
    #[default]
    Normal,
}

impl From<MapType> for u32 {
    fn from(value: MapType) -> Self {
        match value {
            MapType::Meta => MAP_META | MAP_MAPPED,
            MapType::Normal => MAP_MAPPED,
        }
    }
}

pub(crate) type MapResult = PosixResult<Map>;

/// Iterates over the data map represented by an inode.
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
            let result = self.sbi.map(self.inode, self.offset);
            match result {
                Ok(mut m) => {
                    let ba = DiskBlockAccessor::new(self.sbi.superblock(), m.physical.start);
                    let len = m.physical.len.min(ba.len);
                    m.physical.len = len;
                    m.logical.len = len;
                    self.offset += len;
                    Some(m)
                }
                Err(_) => None,
            }
        }
    }
}
