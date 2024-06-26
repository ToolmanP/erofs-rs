use core::mem::transmute;

use crate::compression::CompressionInfo;

pub const EROFS_ISLOTBITS: u8 = 5;

#[allow(non_camel_case_types)]
pub type erofs_blk_t = u32;

#[allow(non_camel_case_types)]
pub type erofs_off_t = u64;

#[allow(non_camel_case_types)]
pub type erofs_nid_t = u64;

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct SuperBlock {
    pub magic: i32,
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
            islotbits: EROFS_ISLOTBITS,
        }
    }
}

impl From<SuperBlockInfo> for SuperBlock {
    fn from(value: SuperBlockInfo) -> Self {
        value.sb
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_superblock_size() {
        assert_eq!(core::mem::size_of::<SuperBlock>(), 128);
    }
}
