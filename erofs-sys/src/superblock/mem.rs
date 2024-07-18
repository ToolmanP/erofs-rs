// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use super::*;
use core::marker::PhantomData;

pub struct MemFileSystem<'a, T>
// Memory Mapped Device/File so we need to have some life
where
    T: MemoryBackend<'a>,
{
    backend: T,
    sb: SuperBlock,
    _marker: PhantomData<dyn MemorySource<'a>>,
}
impl<'a, T> SuperBlockInfo<'a, T> for MemFileSystem<'a, T>
where
    T: MemoryBackend<'a>,
{
    fn superblock(&self) -> &SuperBlock {
        &self.sb
    }
    fn backend(&self) -> &T {
        &self.backend
    }
    fn content_iter(&'a self, inode: &Inode) -> impl Iterator<Item = impl Buffer> {
        RefIter::new(&self.backend, MapIter::new(self, inode))
    }
}
