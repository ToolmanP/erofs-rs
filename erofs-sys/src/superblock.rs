// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use crate::data::*;
use crate::dir::*;
use crate::inode::*;
use crate::map::*;
use crate::*;
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

pub(crate) trait SuperBlockInfo<'a, T>
where
    T: Backend,
{
    fn superblock(&self) -> &SuperBlock;
    fn backend(&self) -> &T;
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

    fn read_inode(&self, nid: Nid) -> Inode {
        let offset = self.iloc(nid);
        let mut buf: InodeBuf = DEFAULT_INODE_BUF;
        self.backend().fill(&mut buf, offset).unwrap();
        Inode {
            inner: GenericInode::try_from(buf).unwrap(),
            nid,
        }
    }

    fn flatmap(&self, inode: &Inode, offset: Off) -> Map {
        let layout = inode.inner.format().layout();
        let nblocks = self.blk_round_up(inode.inner.file_size());

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
                    let len = inode.inner.file_size() - offset;
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

    fn map(&self, inode: &Inode, offset: Off) -> Map {
        self.flatmap(inode, offset)
    }

    fn content_iter(&'a self, inode: &Inode) -> impl Iterator<Item = impl Buffer>;

    fn fill_dentries(&'a self, inode: &Inode, emitter: impl Fn(Dirent) -> ()) {
        for buf in self.content_iter(inode) {
            for dirent in DirCollection::new(buf.content()) {
                emitter(dirent)
            }
        }
    }

    fn find_nid(&'a self, inode: &Inode, name: &str) -> Option<Nid> {
        for buf in self.content_iter(inode) {
            for dirent in DirCollection::new(buf.content()) {
                if dirent.dirname() == name.as_bytes() {
                    return Some(dirent.desc.nid);
                }
            }
        }
        None
    }

    fn ilookup(&'a self, name: &str) -> Option<Inode> {
        let mut nid = self.superblock().root_nid as Nid;
        for part in name.split('/') {
            if part.is_empty() {
                continue;
            }
            let inode = self.read_inode(nid);
            nid = self.find_nid(&inode, part)?;
        }
        Some(self.read_inode(nid))
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::fs::File;
    use std::path::Path;

    pub(crate) fn load_fixture() -> File {
        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sample.img"));
        let file = File::open(path);
        assert!(file.is_ok());
        file.unwrap()
    }

    #[test]
    fn test_superblock_size() {
        assert_eq!(core::mem::size_of::<SuperBlock>(), 128);
    }
}
