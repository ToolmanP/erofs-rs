use crate::data::*;
use crate::inode::*;
use crate::map::*;
use crate::*;

pub mod file;
pub mod mem;

pub const ISLOTBITS: u8 = 5;
pub const SB_MAGIC: u32 = 0xE0F5E1E2;

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
        self.blkpos(sb.meta_blkaddr as u32) + ((nid as Off) << (sb.meta_blkaddr as Off))
    }

    fn read_inode(&self, nid: Nid) -> Inode {
        let offset = self.iloc(nid);
        let mut buf: InodeBuf = DEFAULT_INODE_BUF;
        self.backend()
            .fill(&mut buf, offset, core::mem::size_of::<InodeBuf>() as u64)
            .unwrap();
        Inode {
            inner: GenericInode::try_from(buf).unwrap(),
            nid,
        }
    }

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

    fn map(&self, inode: &Inode, offset: Off) -> Map {
        self.flatmap(inode, offset)
    }

    fn find_nid(&'a self, inode: &Inode, name: &str) -> Option<Nid>;

    fn ilookup(&'a self, name: &str) -> Option<Inode> {
        let mut nid = self.superblock().root_nid as Nid;
        for part in name.split('/') {
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
    use std::os::unix::fs::FileExt;
    use std::path::Path;

    fn load_fixture() -> File {
        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sample.img"));
        let file = File::open(path);
        assert!(file.is_ok());
        file.unwrap()
    }

    #[test]
    fn test_superblock_size() {
        assert_eq!(core::mem::size_of::<SuperBlock>(), 128);
    }

    #[test]
    fn test_superblock_def() {
        let img = load_fixture();
        let mut buf: [u8; 128] = [0; 128];
        img.read_exact_at(&mut buf, 1024).unwrap();
        let superblock = SuperBlock::from(buf);
        assert_eq!(superblock.magic, SB_MAGIC);
    }
}
