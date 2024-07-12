pub mod uncompressed;

use core::marker::PhantomData;

use crate::dir::*;
use crate::inode::*;
use crate::map::*;
use crate::superblock::SuperBlockInfo;
use crate::*;

#[derive(Debug)]
pub(crate) enum SourceError {
    Dummy,
}

#[derive(Debug)]
pub(crate) enum BackendError {
    Dummy,
}

pub(crate) type SourceResult<T> = Result<T, SourceError>;
pub(crate) type BackendResult<T> = Result<T, BackendError>;

pub(crate) trait Source {
    fn fill(&self, data: &mut [u8], offset: Off) -> SourceResult<()>;
    fn get_block(&self, offset: Off) -> SourceResult<Block>;
}

pub(crate) trait FileSource: Source {}

pub(crate) trait MemorySource<'a>: Source {
    fn as_ref_block(&'a self, offset: Off) -> SourceResult<&'a Block>;
}

pub(crate) trait Backend {
    fn fill(&self, data: &mut [u8], offset: Off) -> BackendResult<()>;
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

pub(crate) struct MapIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: Backend,
{
    sbi: &'a T,
    inode: &'b Inode,
    offset: Off,
    len: Off,
    _marker: PhantomData<&'a U>,
}

impl<'a, 'b, T, U> MapIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: Backend,
{
    pub fn new(sbi: &'a T, inode: &'b Inode) -> Self {
        Self {
            sbi,
            inode,
            offset: 0,
            len: inode.inner.size(),
            _marker: Default::default(),
        }
    }
}

impl<'a, 'b, T, U> Iterator for MapIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: Backend,
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

pub(crate) struct BlockIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: FileBackend,
{
    backend: &'a U,
    map_iter: MapIter<'a, 'b, T, U>,
}

impl<'a, 'b, T, U> BlockIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: FileBackend,
{
    pub(crate) fn new(backend: &'a U, map_iter: MapIter<'a, 'b, T, U>) -> Self {
        Self { backend, map_iter }
    }
    pub fn find_nid(self, name: &str) -> Option<Nid> {
        for block in self {
            for dirent in DirCollection::new(&block) {
                let dirname = dirent.dirname(&block);
                if dirname == name.as_bytes() {
                    return Some(dirent.desc.nid as u64);
                }
            }
        }
        None
    }
}

impl<'a, 'b, T, U> Iterator for BlockIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: FileBackend,
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

pub(crate) struct BlockRefIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: MemoryBackend<'a>,
{
    backend: &'a U,
    map_iter: MapIter<'a, 'b, T, U>,
}

impl<'a, 'b, T, U> BlockRefIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: MemoryBackend<'a>,
{
    pub(crate) fn new(backend: &'a U, map_iter: MapIter<'a, 'b, T, U>) -> Self {
        Self { backend, map_iter }
    }

    pub(crate) fn find_nid(self, name: &str) -> Option<Nid> {
        for block in self {
            for dirent in DirCollection::new(block) {
                let dirname = dirent.dirname(block);
                if dirname == name.as_bytes() {
                    return Some(dirent.desc.nid as u64);
                }
            }
        }
        None
    }
}

impl<'a, 'b, T, U> Iterator for BlockRefIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: MemoryBackend<'a>,
{
    type Item = &'a Block;
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
