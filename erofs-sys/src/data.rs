pub mod uncompressed;
use crate::inode::{Inode, Layout, Spec};
use crate::map::*;
use crate::superblock::SuperBlockInfo;
use crate::*;

#[derive(Debug)]
pub enum SourceError {
    Dummy,
}

#[derive(Debug)]
pub enum BackendError {
    Dummy,
}

type SourceResult<T> = Result<T, SourceError>;
type BackendResult<T> = Result<T, BackendError>;

pub(crate) trait Source {
    fn fill(&self, data: &mut [u8], offset: Off, len: Off) -> SourceResult<Off>;
    fn get_block(&self, offset: Off, len: Off) -> SourceResult<Block>;
}

pub(crate) trait FileSource: Source {}

pub(crate) trait MemorySource<'a>: Source {
    fn as_ref_block(&'a self, offset: Off) -> SourceResult<&'a Block>;
}

pub(crate) trait Backend {
    fn fill(&self, data: &mut [u8], offset: Off, len: Off) -> BackendResult<Off>;
    fn get_block(&self, offset: Off) -> BackendResult<Block>;
}

pub(crate) trait FileBackend: Backend {}

pub(crate) trait MemoryBackend<'a>: Backend {
    fn as_ref_block(&'a self, offset: Off) -> BackendResult<&'a Block>;
}

pub(crate) trait Buffer<'a> {
    fn raw(&'a mut self, offset: Off, len: Off) -> Option<&'a mut [u8]>;
    fn buflen(&self) -> Off;
}

impl<'a> Buffer<'a> for Block {
    fn raw(&'a mut self, offset: Off, len: Off) -> Option<&'a mut [u8]> {
        if (offset + len) as usize >= EROFS_BLOCK_SZ {
            None
        } else {
            Some(&mut self[offset as usize..(offset + len) as usize])
        }
    }
    fn buflen(&self) -> Off {
        EROFS_BLOCK_SZ as u64
    }
}

impl<'a> Buffer<'a> for [u8] {
    fn raw(&'a mut self, offset: Off, len: Off) -> Option<&'a mut [u8]> {
        if ((offset + len) as u64) < self.len() as u64 {
            Some(&mut self[offset as usize..(offset + len) as usize])
        } else {
            None
        }
    }
    fn buflen(&self) -> Off {
        self.len() as Off
    }
}

impl<'a, 'b, T> SuperBlockInfo<T>
where
    T: Backend,
    'a: 'b,
{
    fn flatmap(&self, inode: &Inode, offset: Off) -> Map {
        let layout = inode.inner.format().layout();
        let nblocks = self.blk_round_up(inode.inner.size());

        let blkaddr = match inode.inner.spec() {
            Spec::Data(blkaddr) => blkaddr,
            _ => unimplemented!(),
        };

        let lastblk = match layout {
            Layout::FlatInline => nblocks - 1,
            _ => nblocks,
        };

        if offset < self.blkpos(lastblk) {
            let len = self.blkpos(lastblk) - offset;
            Map {
                index: 0,
                offset: 0,
                logical: AddressMap { start: offset, len },
                physical: AddressMap {
                    start: self.blkpos(blkaddr) + offset,
                    len,
                },
            }
        } else {
            match layout {
                Layout::FlatInline => {
                    let len = inode.inner.inode_size() - offset;
                    Map {
                        index: 0,
                        offset: 0,
                        logical: AddressMap { start: offset, len },
                        physical: AddressMap {
                            start: self.iloc(inode.nid)
                                + inode.inner.inode_size()
                                + inode.inner.xattr_size()
                                + self.blkoff(offset),
                            len,
                        },
                    }
                }
                _ => unimplemented!(),
            }
        }
    }

    pub(crate) fn map(&self, inode: &Inode, offset: Off) -> Map {
        self.flatmap(inode, offset)
    }

    fn iter_map(&'a self, inode: &'b Inode) -> MapIter<'a, 'b, T> {
        MapIter::new(self, inode)
    }
}

impl<'a, 'b, T> SuperBlockInfo<T>
where
    T: FileBackend,
    'a: 'b,
{
    pub(crate) fn block_iter(&'a self, inode: &'b Inode) -> BlockIter<'a, 'b, T> {
        BlockIter::new(&self.backend, self.iter_map(inode))
    }
}

impl<'a, 'b, T> SuperBlockInfo<T>
where
    T: MemoryBackend<'a>,
    'a: 'b,
{
    pub(crate) fn block_ref_iter(&'a self, inode: &'b Inode) -> BlockRefIter<'a, 'b, T> {
        BlockRefIter::new(&self.backend, self.iter_map(inode))
    }
}

struct MapIter<'a, 'b, T>
where
    T: Backend,
    'a: 'b,
{
    sbi: &'a SuperBlockInfo<T>,
    inode: &'b Inode,
    offset: Off,
    len: Off,
}

impl<'a, 'b, T> MapIter<'a, 'b, T>
where
    T: Backend,
    'a: 'b,
{
    pub fn new(sbi: &'a SuperBlockInfo<T>, inode: &'b Inode) -> Self {
        Self {
            sbi,
            inode,
            offset: 0,
            len: inode.inner.size(),
        }
    }
}

impl<'a, 'b, T> Iterator for MapIter<'a, 'b, T>
where
    T: Backend,
    'a: 'b,
{
    type Item = Map;
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset == self.len {
            None
        } else {
            let m = self.sbi.map(self.inode, self.offset);
            self.offset += m.logical.len.min(EROFS_BLOCK_SZ as u64);
            Some(m)
        }
    }
}

pub(crate) struct BlockIter<'a, 'b, T>
where
    T: Backend,
    'a: 'b,
{
    backend: &'a T,
    map_iter: MapIter<'a, 'b, T>,
}

impl<'a, 'b, T> BlockIter<'a, 'b, T>
where
    T: FileBackend,
    'a: 'b,
{
    pub(crate) fn new(backend: &'a T, map_iter: MapIter<'a, 'b, T>) -> Self {
        Self { backend, map_iter }
    }
}

impl<'a, 'b, T> Iterator for BlockIter<'a, 'b, T>
where
    T: FileBackend,
    'a: 'b,
{
    type Item = Block;
    fn next(&mut self) -> Option<Self::Item> {
        match self.map_iter.next() {
            Some(m) => match self.backend.get_block(m.physical.start) {
                Ok(block) => Some(block),
                Err(_) => None,
            },
            None => None,
        }
    }
}

pub(crate) struct BlockRefIter<'a, 'b, T>
where
    T: MemoryBackend<'a>,
    'a: 'b,
{
    backend: &'a T,
    map_iter: MapIter<'a, 'b, T>,
}

impl<'a, 'b, T> BlockRefIter<'a, 'b, T>
where
    T: MemoryBackend<'a>,
    'a: 'b,
{
    pub(crate) fn new(backend: &'a T, map_iter: MapIter<'a, 'b, T>) -> Self {
        Self { backend, map_iter }
    }
}

impl<'a, 'b, T> Iterator for BlockRefIter<'a, 'b, T>
where
    T: MemoryBackend<'a>,
    'a: 'b,
{
    type Item = &'b Block;
    fn next(&mut self) -> Option<Self::Item> {
        match self.map_iter.next() {
            Some(m) => match self.backend.as_ref_block(m.physical.start) {
                Ok(block) => Some(block),
                Err(_) => None,
            },
            None => None,
        }
    }
}
