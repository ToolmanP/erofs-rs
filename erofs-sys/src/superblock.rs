// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use alloc::boxed::Box;

use super::data::*;
use super::dir::*;
use super::inode::*;
use super::map::*;
use super::*;
use core::mem::size_of;

pub mod file;
pub mod mem;

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct SuperBlock {
    pub(crate) magic: u32,
    pub(crate) checksum: i32,
    pub(crate) feature_compat: i32,
    pub(crate) blkszbits: u8,
    pub(crate) sb_extslots: u8,
    pub(crate) root_nid: i16,
    pub(crate) inos: i64,
    pub(crate) build_time: i64,
    pub(crate) build_time_nsec: i32,
    pub(crate) blocks: i32,
    pub(crate) meta_blkaddr: i32,
    pub(crate) uuid: [u8; 16],
    pub(crate) volume_name: [u8; 16],
    pub(crate) feature_incompat: i32,
    pub(crate) compression: i32,
    pub(crate) extra_devices: i16,
    pub(crate) devt_slotoff: i16,
    pub(crate) dirblkbits: u8,
    pub(crate) xattr_prefix_count: u8,
    pub(crate) xattr_prefix_start: i32,
    pub(crate) packed_nid: i64,
    pub(crate) xattr_filter_reserved: u8,
    pub(crate) reserved: [u8; 23],
}
// SAFETY: SuperBlock uses all ffi-safe types.
impl From<&[u8]> for SuperBlock {
    fn from(value: &[u8]) -> Self {
        unsafe { *(value.as_ptr() as *const SuperBlock) }
    }
}

// SAFETY: SuperBlock uses all ffi-safe types.
impl From<[u8; 128]> for SuperBlock {
    fn from(value: [u8; 128]) -> Self {
        unsafe { *(value.as_ptr() as *const SuperBlock) }
    }
}

// SAFETY: SuperBlock uses all ffi-safe types.
impl From<SuperBlock> for [u8; 128] {
    fn from(value: SuperBlock) -> Self {
        unsafe { core::mem::transmute(value) }
    }
}

pub(crate) type SuperBlockBuf = [u8; size_of::<SuperBlock>()];
pub(crate) const SUPERBLOCK_EMPTY_BUF: SuperBlockBuf = [0; size_of::<SuperBlock>()];

pub(crate) trait FileSystem<I>
where
    I: Inode,
{
    fn superblock(&self) -> &SuperBlock;
    fn backend(&self) -> &dyn Backend;
    fn blknr(&self, pos: Off) -> Blk {
        (pos >> self.superblock().blkszbits) as Blk
    }

    fn blkpos(&self, blk: Blk) -> Off {
        (blk as Off) << self.superblock().blkszbits
    }

    fn blkoff(&self, offset: Off) -> Off {
        offset & (self.blksz() - 1)
    }

    fn blksz(&self) -> Off {
        1 << self.superblock().blkszbits
    }

    fn blk_round_up(&self, addr: Off) -> Blk {
        ((addr + self.blksz() - 1) >> self.superblock().blkszbits) as Blk
    }

    fn iloc(&self, nid: Nid) -> Off {
        let sb = &self.superblock();
        self.blkpos(sb.meta_blkaddr as u32) + ((nid as Off) << (5 as Off))
    }

    fn read_inode_info(&self, nid: Nid) -> InodeInfo {
        let offset = self.iloc(nid);
        let mut buf: InodeInfoBuf = DEFAULT_INODE_BUF;
        self.backend().fill(&mut buf, offset).unwrap();
        InodeInfo::try_from(buf).unwrap()
    }

    fn flatmap(&self, inode: &I, offset: Off) -> Map {
        let layout = inode.info().format().layout();
        let nblocks = self.blk_round_up(inode.info().file_size());

        let blkaddr = match inode.info().spec() {
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
                    let len = inode.info().file_size() - offset;
                    Map {
                        index: 0,
                        offset: 0,
                        logical: AddressMap { start: offset, len },
                        physical: AddressMap {
                            start: self.iloc(*inode.nid())
                                + inode.info().inode_size()
                                + inode.info().xattr_size()
                                + self.blkoff(offset),
                            len,
                        },
                    }
                }
                _ => unimplemented!(),
            }
        }
    }

    fn map(&self, inode: &I, offset: Off) -> Map {
        self.flatmap(inode, offset)
    }

    // TODO:: Remove the Box<dyn Iterator> here
    // Maybe create another wrapper type and we implement the Iterator there?
    // Seems unachievable because of static dispatch of Buffer is not allowed at compile time
    // If we want to have trait object that can be exported to c_void
    // Leave it as it is for tradeoffs

    fn content_iter<'b, 'a: 'b>(
        &'a self,
        inode: &'b I,
    ) -> Box<dyn Iterator<Item = Box<dyn Buffer + 'b>> + 'b>;

    fn fill_dentries(&self, inode: &I, emitter: &dyn Fn(Dirent)) {
        for buf in self.content_iter(inode) {
            for dirent in DirCollection::new(buf.content()) {
                emitter(dirent)
            }
        }
    }

    fn find_nid(&self, inode: &I, name: &str) -> Option<Nid> {
        for buf in self.content_iter(inode) {
            for dirent in DirCollection::new(buf.content()) {
                if dirent.dirname() == name.as_bytes() {
                    return Some(dirent.desc.nid);
                }
            }
        }
        None
    }
}

pub struct BufferedFileSystem<I, C>
where
    I: Inode,
    C: InodeCollection<I = I>,
{
    filesystem: Box<dyn FileSystem<I>>,
    collection: C,
}

impl<I, C> BufferedFileSystem<I, C>
where
    I: Inode,
    C: InodeCollection<I = I>,
{
    fn new(fs: Box<dyn FileSystem<I>>, c: C) -> Self {
        Self {
            filesystem: fs,
            collection: c,
        }
    }

    fn read_inode<'a>(
        filesystem: &'a Box<dyn FileSystem<I>>,
        collection: &'a mut C,
        nid: Nid,
    ) -> &'a mut I {
        let (inode, is_init) = collection.iget(nid);
        if !is_init {
            inode.write(I::new(filesystem.read_inode_info(nid), nid));
        }
        unsafe { inode.assume_init_mut() }
    }

    fn superblock(&self) -> &SuperBlock {
        self.filesystem.superblock()
    }
    fn ilookup<'a>(&'a mut self, name: &str) -> Option<&'a mut I> {
        let mut nid = self.filesystem.superblock().root_nid as Nid;
        for part in name.split('/') {
            if part.is_empty() {
                continue;
            }
            let inode = Self::read_inode(&self.filesystem, &mut self.collection, nid);
            nid = self.filesystem.find_nid(inode, part)?
        }
        Some(Self::read_inode(
            &self.filesystem,
            &mut self.collection,
            nid,
        ))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    extern crate std;

    use super::*;
    use crate::inode::tests::*;
    use core::mem::MaybeUninit;
    use std::collections::HashMap;
    use std::fs::File;
    use std::path::Path;
    pub(crate) const SB_MAGIC: u32 = 0xE0F5E1E2;

    pub(crate) type SimpleBufferedFileSystem =
        BufferedFileSystem<SimpleInode, HashMap<Nid, MaybeUninit<SimpleInode>>>;

    pub(crate) fn load_fixture() -> File {
        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sample.img"));
        let file = File::options().read(true).write(true).open(path);
        assert!(file.is_ok());
        file.unwrap()
    }

    pub(crate) fn test_superblock_def(filesystem: &mut SimpleBufferedFileSystem) {
        assert_eq!(filesystem.superblock().magic, SB_MAGIC);
    }

    pub(crate) fn test_filesystem_ilookup(filesystem: &mut SimpleBufferedFileSystem) {
        let inode = filesystem.ilookup("/texts/lipsum.txt").unwrap();
        assert_eq!(*inode.nid(), 640);
    }

    #[test]
    fn test_superblock_size() {
        assert_eq!(core::mem::size_of::<SuperBlock>(), 128);
    }
}
