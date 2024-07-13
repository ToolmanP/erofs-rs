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
    fn as_ref(&'a self, offset: Off, len: Off) -> SourceResult<&'a [u8]>;
    fn as_mut(&'a mut self, offset: Off, len: Off) -> SourceResult<&'a mut [u8]>;
}

pub(crate) trait Backend {
    fn fill(&self, data: &mut [u8], offset: Off) -> BackendResult<()>;
    fn get_block(&self, offset: Off) -> BackendResult<Block>;
}

pub(crate) trait FileBackend: Backend {}

pub(crate) trait MemoryBackend<'a>: Backend {
    fn as_ref(&'a self, offset: Off, len: Off) -> BackendResult<&'a [u8]>;
    fn as_mut(&'a mut self, offset: Off, len: Off) -> BackendResult<&'a mut [u8]>;
}

pub(crate) struct TempBuffer {
    block: Block,
    maxsize: usize,
}

pub(crate) trait Buffer<'a> {
    fn content_mut(&'a mut self) -> &'a mut [u8];
    fn content(&'a self) -> &'a [u8];
}

impl TempBuffer {
    pub(crate) fn new(block: Block, maxsize: usize) -> Self {
        Self { block, maxsize }
    }
}

impl<'a> Buffer<'a> for TempBuffer {
    fn content_mut(&'a mut self) -> &'a mut [u8] {
        &mut self.block[0..self.maxsize]
    }
    fn content(&'a self) -> &'a [u8] {
        &self.block[0..self.maxsize]
    }
}

impl<'a> Buffer<'a> for [u8] {
    fn content(&'a self) -> &'a [u8] {
        self
    }
    fn content_mut(&'a mut self) -> &'a mut [u8] {
        self
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
            len: inode.inner.file_size(),
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
        if self.offset >= self.len {
            None
        } else {
            let m = self.sbi.map(self.inode, self.offset);
            self.offset += m.logical.len.min(EROFS_BLOCK_SZ as u64);
            Some(m)
        }
    }
}

pub(crate) struct TempBufferIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: FileBackend,
{
    backend: &'a U,
    map_iter: MapIter<'a, 'b, T, U>,
}

impl<'a, 'b, T, U> TempBufferIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: FileBackend,
{
    pub(crate) fn new(backend: &'a U, map_iter: MapIter<'a, 'b, T, U>) -> Self {
        Self { backend, map_iter }
    }
    pub(crate) fn find_nid(&mut self, name: &str) -> Option<Nid> {
        for buf in self.into_iter() {
            for dirent in DirCollection::new(buf.content()) {
                if dirent.dirname(buf.content()) == name.as_bytes() {
                    return Some(dirent.desc.nid);
                }
            }
        }
        None
    }
}


impl<'a, 'b, T, U> Iterator for TempBufferIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: FileBackend,
{
    type Item = TempBuffer;
    fn next(&mut self) -> Option<Self::Item> {
        match self.map_iter.next() {
            Some(m) => {
                if m.logical.len < EROFS_BLOCK_SZ as Off {
                    let mut block = EROFS_EMPTY_BLOCK;
                    match self
                        .backend
                        .fill(&mut block[0..m.physical.len as usize], m.physical.start)
                    {
                        Ok(()) => Some(TempBuffer::new(block, m.physical.len as usize)),
                        Err(_) => None,
                    }
                } else {
                    match self.backend.get_block(m.physical.start) {
                        Ok(block) => Some(TempBuffer::new(block, EROFS_BLOCK_SZ)),
                        Err(_) => None,
                    }
                }
            }
            None => None,
        }
    }
}


pub(crate) struct RefIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: MemoryBackend<'a>,
{
    backend: &'a U,
    map_iter: MapIter<'a, 'b, T, U>,
}

impl<'a, 'b, T, U> RefIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: MemoryBackend<'a>,
{
    pub(crate) fn new(backend: &'a U, map_iter: MapIter<'a, 'b, T, U>) -> Self {
        Self { backend, map_iter }
    }
    pub(crate) fn find_nid(&mut self, name: &str) -> Option<Nid> {
        for buf in self.into_iter() {
            for dirent in DirCollection::new(buf) {
                if dirent.dirname(buf) == name.as_bytes() {
                    return Some(dirent.desc.nid);
                }
            }
        }
        None
    }
}

impl<'a, 'b, T, U> Iterator for RefIter<'a, 'b, T, U>
where
    T: SuperBlockInfo<'a, U>,
    U: MemoryBackend<'a>,
{
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        match self.map_iter.next() {
            Some(m) => match self.backend.as_ref(m.physical.start, m.physical.len) {
                Ok(buf) => Some(buf),
                Err(_) => None,
            },
            None => None,
        }
    }
}
