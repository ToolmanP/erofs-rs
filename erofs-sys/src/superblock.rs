use crate::compression::SuperblockCompressionInfo;
use crate::data::Backend;
use crate::{Blk, Off};

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

#[derive(Debug, Clone, Copy)]
pub(crate) struct SuperBlockInfo<T>
where
    T: Backend,
{
    pub(crate) sb: SuperBlock,
    pub(crate) c_info: SuperblockCompressionInfo,
    pub(crate) islotbits: u8,
    pub(crate) backend: T,
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

impl<T> SuperBlockInfo<T>
where
    T: Backend,
{
    pub(crate) fn new(sb: SuperBlock, backend: T) -> Self {
        Self {
            sb,
            c_info: SuperblockCompressionInfo::default(),
            islotbits: ISLOTBITS,
            backend,
        }
    }
}

impl<T> From<SuperBlockInfo<T>> for SuperBlock
where
    T: Backend,
{
    fn from(value: SuperBlockInfo<T>) -> Self {
        value.sb
    }
}

impl<T> SuperBlockInfo<T>
where
    T: Backend,
{
    pub(crate) fn blknr(&self, pos: Off) -> Blk {
        (pos >> self.sb.blkszbits) as Blk
    }
    pub(crate) fn blkpos(&self, blk: Blk) -> Off {
        (blk as Off) << self.sb.blkszbits
    }
    pub(crate) fn blkoff(&self, offset: Off) -> Off {
        offset & (self.blksz() - 1)
    }
    pub(crate) fn blksz(&self) -> Off {
        1 << self.sb.blkszbits
    }
    pub(crate) fn blk_round_up(&self, addr: Off) -> Blk {
        ((addr + self.blksz() - 1) >> self.sb.blkszbits) as Blk
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
