use crate::*;

use crate::{
    data::{FileBackend, MemoryBackend},
    inode::{Inode, Type},
    superblock::SuperBlockInfo,
};

#[repr(C, packed)]
pub(crate) struct DirentDesc {
    pub nid: u32,
    pub nameoff: u16,
    pub file_type: u8,
    pub reserved: u8,
}

pub(crate) struct Dirent<'a> {
    pub desc: &'a DirentDesc,
    pub len: usize,
}

pub(crate) struct DirCollection<'a> {
    block: &'a Block,
    offset: usize,
    total: usize,
}

impl<'a> DirCollection<'a> {
    pub(crate) fn new(block: &'a Block) -> Self {
        let desc: &'a DirentDesc = unsafe { &*(block.as_ptr() as *const DirentDesc) };
        Self {
            block,
            offset: 0,
            total: desc.nameoff as usize / core::mem::size_of::<DirentDesc>(),
        }
    }
    pub(crate) fn dirent(&self, index: usize) -> Option<Dirent<'a>> {
        //SAFETY: Note that DirentDesc is yet another ffi-safe type and the size of Block is larger
        //than that of DirentDesc. It's safe to allow this unsafe cast.
        let descs: &'a [DirentDesc] = unsafe {
            core::slice::from_raw_parts(self.block.as_ptr() as *const DirentDesc, self.total)
        };
        if index >= self.total {
            None
        } else if index == self.total - 1 {
            let len = EROFS_BLOCK_SZ - descs[self.total - 1].nameoff as usize;
            Some(Dirent {
                desc: &descs[index],
                len,
            })
        } else {
            let len = (descs[index + 1].nameoff - descs[index].nameoff) as usize;
            Some(Dirent {
                desc: &descs[index],
                len,
            })
        }
    }
}

impl<'a> Iterator for DirCollection<'a> {
    type Item = Dirent<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.dirent(self.offset).map(|x| {
            self.offset += 1;
            x
        })
    }
}

impl<'a> Dirent<'a> {
    pub(crate) fn dirname(&self, block: &'a Block) -> &'a [u8] {
        let nameoff = self.desc.nameoff as usize;
        &block[nameoff..nameoff + self.len]
    }
}
