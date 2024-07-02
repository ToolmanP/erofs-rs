use core::mem::transmute;

use crate::compression::CompressionInfo;
use crate::inode::Inode;
use crate::{Blk, Offset};

pub const ISLOTBITS: u8 = 5;
pub const SB_MAGIC: u32 = 0xE0F5E1E2;


#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct SuperBlock {
    pub magic: u32,
    pub checksum: i32,
    pub feature_compat: i32,
    pub blkszbits: u8,
    pub sb_extslots: u8,
    pub root_nid: i16,
    pub inos: i64,
    pub build_time: i64,
    pub build_time_nsec: i32,
    pub blocks: i32,
    pub meta_blkaddr: i32,
    pub uuid: [u8; 16],
    pub volume_name: [u8; 16],
    pub feature_incompat: i32,
    pub compression: i32,
    pub extra_devices: i16,
    pub devt_slotoff: i16,
    pub dirblkbits: u8,
    pub xattr_prefix_count: u8,
    pub xattr_prefix_start: i32,
    pub packed_nid: i64,
    pub xattr_filter_reserved: u8,
    pub reserved: [u8; 23],
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SuperBlockInfo {
    pub sb: SuperBlock,
    pub c_info: CompressionInfo,
    pub islotbits: u8,
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
        unsafe { transmute(value) }
    }
}

impl From<SuperBlock> for SuperBlockInfo {
    fn from(value: SuperBlock) -> Self {
        Self {
            sb: value,
            c_info: CompressionInfo::default(),
            islotbits: ISLOTBITS,
        }
    }
}

impl From<SuperBlockInfo> for SuperBlock {
    fn from(value: SuperBlockInfo) -> Self {
        value.sb
    }
}

impl SuperBlockInfo {
    pub fn blknr(&self, pos: Offset) -> Blk {
        (pos >> self.sb.blkszbits) as Blk
    }
    pub fn blkpos(&self, blk: Blk) -> Offset {
        (blk as Offset) << self.sb.blkszbits
    }
    pub fn iloc(&self, inode: &Inode) -> Offset {
        let sb: &SuperBlock = &self.sb;
        self.blkpos(sb.meta_blkaddr as u32) + ((inode.nid as Offset) << (sb.meta_blkaddr as Offset))
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
