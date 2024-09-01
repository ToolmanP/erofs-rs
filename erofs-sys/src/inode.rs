// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

use super::superblock::*;
use super::xattrs::*;
use super::*;
use core::mem::size_of;

/// Represents the compact bitfield of the Erofs Inode format.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct Format(u16);

/// The Version of the Inode which represents whether this inode is extended or compact.
/// Extended inodes have more infos about nlinks + mtime.
/// This is documented in https://erofs.docs.kernel.org/en/latest/core_ondisk.html#inodes
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) enum Version {
    Compat,
    Extended,
    Unknown,
}

/// Represents the data layout backed by the Inode.
/// As Documented in https://erofs.docs.kernel.org/en/latest/core_ondisk.html#inode-data-layouts
#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Layout {
    FlatPlain = 0,
    CompressedFull = 1,
    FlatInline = 2,
    CompressedCompact = 3,
    Chunk = 4,
    Unknown = 5,
}

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Type {
    Regular,
    Directory,
    Link,
    Character,
    Block,
    Fifo,
    Socket,
    Unknown,
}

/// This is format extracted from i_format bit representation.
/// This includes various infos and specs about the inode.
impl Format {
    pub(crate) fn version(&self) -> Version {
        match (self.0) & ((1 << 1) - 1) {
            0 => Version::Compat,
            1 => Version::Extended,
            _ => Version::Unknown,
        }
    }

    pub(crate) fn layout(&self) -> Layout {
        match (self.0 >> 1) & ((1 << 3) - 1) {
            0 => Layout::FlatPlain,
            1 => Layout::CompressedFull,
            2 => Layout::FlatInline,
            3 => Layout::CompressedCompact,
            4 => Layout::Chunk,
            _ => Layout::Unknown,
        }
    }
}

/// Represents the compact inode which resides on-disk.
/// This is documented in https://erofs.docs.kernel.org/en/latest/core_ondisk.html#inodes
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct CompactInodeInfo {
    pub(crate) i_format: Format,
    pub(crate) i_xattr_icount: u16,
    pub(crate) i_mode: u16,
    pub(crate) i_nlink: u16,
    pub(crate) i_size: u32,
    pub(crate) i_reserved: [u8; 4],
    pub(crate) i_u: [u8; 4],
    pub(crate) i_ino: u32,
    pub(crate) i_uid: u16,
    pub(crate) i_gid: u16,
    pub(crate) i_reserved2: [u8; 4],
}

/// Represents the extended inode which resides on-disk.
/// This is documented in https://erofs.docs.kernel.org/en/latest/core_ondisk.html#inodes
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ExtendedInodeInfo {
    pub(crate) i_format: Format,
    pub(crate) i_xattr_icount: u16,
    pub(crate) i_mode: u16,
    pub(crate) i_reserved: [u8; 2],
    pub(crate) i_size: u64,
    pub(crate) i_u: [u8; 4],
    pub(crate) i_ino: u32,
    pub(crate) i_uid: u32,
    pub(crate) i_gid: u32,
    pub(crate) i_mtime: u64,
    pub(crate) i_mtime_nsec: u32,
    pub(crate) i_nlink: u32,
    pub(crate) i_reserved2: [u8; 16],
}

/// Represents the inode info which is either compact or extended.
#[derive(Clone, Copy)]
pub(crate) enum InodeInfo {
    Extended(ExtendedInodeInfo),
    Compact(CompactInodeInfo),
}

pub(crate) const CHUNK_BLKBITS_MASK: u16 = 0x1f;
pub(crate) const CHUNK_FORMAT_INDEX_BIT: u16 = 0x20;

/// Represents on-disk chunk index of the file backing inode.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ChunkIndex {
    pub(crate) advise: u16,
    pub(crate) device_id: u16,
    pub(crate) blkaddr: u32,
}

impl From<[u8; 8]> for ChunkIndex {
    fn from(u: [u8; 8]) -> Self {
        let advise = u16::from_le_bytes([u[0], u[1]]);
        let device_id = u16::from_le_bytes([u[2], u[3]]);
        let blkaddr = u32::from_le_bytes([u[4], u[5], u[6], u[7]]);
        ChunkIndex {
            advise,
            device_id,
            blkaddr,
        }
    }
}

/// Chunk format used for indicating the chunkbits and chunkindex.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ChunkFormat(pub(crate) u16);

impl ChunkFormat {
    pub(crate) fn is_chunkindex(&self) -> bool {
        self.0 & CHUNK_FORMAT_INDEX_BIT != 0
    }
    pub(crate) fn chunkbits(&self) -> u16 {
        self.0 & CHUNK_BLKBITS_MASK
    }
}

/// Chunk description which represents whether this file consists of compound chunks or raw chunks;
#[derive(Clone, Copy, Debug)]
pub(crate) enum ChunkDesc {
    ChunkIndex(ChunkIndex),
    RawPos(Blk),
}

/// Represents the data spec of the inode which is either consequentive raw blocks or in sparse chunk format.
#[derive(Clone, Copy, Debug)]
pub(crate) enum DataSpec {
    RawBlk(u32),
    Chunk(ChunkFormat),
}

/// Represents the inode spec which is either data or device.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Spec {
    Data(DataSpec),
    Device(u32),
    Unknown,
}

/// Convert the spec from the format of the inode based on the layout.
impl Spec {
    pub(crate) fn data(u: &[u8; 4], layout: Layout) -> Self {
        match layout {
            Layout::FlatInline | Layout::FlatPlain => {
                Spec::Data(DataSpec::RawBlk(u32::from_le_bytes(*u)))
            }
            Layout::Chunk => {
                let chunkformat = u16::from_le_bytes([u[0], u[1]]);
                Spec::Data(DataSpec::Chunk(ChunkFormat(chunkformat)))
            }
            // We don't support compressed inlines or compressed chunks currently.
            _ => Spec::Unknown,
        }
    }
}

/// Helper functions for Inode Info.
impl InodeInfo {
    pub(crate) fn ino(&self) -> u32 {
        match self {
            Self::Extended(extended) => extended.i_ino,
            Self::Compact(compact) => compact.i_ino,
        }
    }

    pub(crate) fn format(&self) -> Format {
        match self {
            Self::Extended(extended) => extended.i_format,
            Self::Compact(compact) => compact.i_format,
        }
    }

    pub(crate) fn file_size(&self) -> Off {
        match self {
            Self::Extended(extended) => extended.i_size,
            Self::Compact(compact) => compact.i_size as u64,
        }
    }

    pub(crate) fn inode_size(&self) -> Off {
        match self {
            Self::Extended(_) => 64,
            Self::Compact(_) => 32,
        }
    }

    pub(crate) fn xattr_size(&self) -> Off {
        match self {
            Self::Extended(extended) => 12 + 4 * (extended.i_xattr_icount as u64 - 1),
            Self::Compact(_) => 0,
        }
    }

    pub(crate) fn xattr_count(&self) -> u16 {
        match self {
            Self::Extended(extended) => extended.i_xattr_icount,
            Self::Compact(compact) => compact.i_xattr_icount,
        }
    }

    pub(crate) fn spec(&self) -> Spec {
        let mode = match self {
            Self::Extended(extended) => extended.i_mode,
            Self::Compact(compact) => compact.i_mode,
        };

        let u = match self {
            Self::Extended(extended) => &extended.i_u,
            Self::Compact(compact) => &compact.i_u,
        };

        match mode & 0o170000 {
            0o40000 => Spec::data(u, self.format().layout()),
            0o100000 => Spec::data(u, self.format().layout()),
            0o120000 => Spec::data(u, self.format().layout()), // Real Data
            // We don't support device inodes currently.
            _ => Spec::Unknown,
        }
    }

    pub(crate) fn inode_type(&self) -> Type {
        let mode = match self {
            Self::Extended(extended) => extended.i_mode,
            Self::Compact(compact) => compact.i_mode,
        };
        match mode & 0o170000 {
            0o40000 => Type::Directory, // Directory
            0o100000 => Type::Regular,  // Regular File
            0o120000 => Type::Link,     // Symbolic Link
            0o10000 => Type::Fifo,      // FIFO
            0o140000 => Type::Socket,   // Socket
            0o60000 => Type::Block,     // Block
            0o20000 => Type::Character, // Character
            _ => Type::Unknown,
        }
    }
}

pub(crate) type CompactInodeInfoBuf = [u8; size_of::<CompactInodeInfo>()];
pub(crate) type ExtendedInodeInfoBuf = [u8; size_of::<ExtendedInodeInfo>()];
pub(crate) const DEFAULT_INODE_BUF: ExtendedInodeInfoBuf = [0; size_of::<ExtendedInodeInfo>()];

/// The inode trait which represents the inode in the filesystem.
pub(crate) trait Inode: Sized {
    fn new(
        _sb: &SuperBlock,
        info: InodeInfo,
        nid: Nid,
        xattrs_shared_entries: XAttrSharedEntries,
    ) -> Self;
    fn info(&self) -> &InodeInfo;
    fn xattrs_shared_entries(&self) -> &XAttrSharedEntries;
    fn nid(&self) -> Nid;
}

/// Represents the error which occurs when trying to convert the inode.
#[derive(Debug)]
pub(crate) enum InodeError {
    VersionError,
    PosixError(Errno),
}

impl TryFrom<CompactInodeInfoBuf> for CompactInodeInfo {
    type Error = InodeError;
    fn try_from(value: CompactInodeInfoBuf) -> Result<Self, Self::Error> {
        let inode: CompactInodeInfo = Self {
            i_format: Format(u16::from_le_bytes(value[0..2].try_into().unwrap())),
            i_xattr_icount: u16::from_le_bytes(value[2..4].try_into().unwrap()),
            i_mode: u16::from_le_bytes(value[4..6].try_into().unwrap()),
            i_nlink: u16::from_le_bytes(value[6..8].try_into().unwrap()),
            i_size: u32::from_le_bytes(value[8..12].try_into().unwrap()),
            i_reserved: value[12..16].try_into().unwrap(),
            i_u: value[16..20].try_into().unwrap(),
            i_ino: u32::from_le_bytes(value[20..24].try_into().unwrap()),
            i_uid: u16::from_le_bytes(value[24..26].try_into().unwrap()),
            i_gid: u16::from_le_bytes(value[26..28].try_into().unwrap()),
            i_reserved2: value[28..32].try_into().unwrap(),
        };
        let ifmt = &inode.i_format;
        match ifmt.version() {
            Version::Compat => Ok(inode),
            Version::Extended => Err(InodeError::VersionError),
            _ => Err(InodeError::PosixError(Errno::EOPNOTSUPP)),
        }
    }
}

impl TryFrom<ExtendedInodeInfoBuf> for InodeInfo {
    type Error = Errno;
    fn try_from(value: ExtendedInodeInfoBuf) -> Result<Self, Self::Error> {
        let compact_buf: CompactInodeInfoBuf = value[0..32].try_into().unwrap();
        let r: Result<CompactInodeInfo, InodeError> = CompactInodeInfo::try_from(compact_buf);
        match r {
            Ok(compact) => Ok(InodeInfo::Compact(compact)),
            Err(e) => match e {
                InodeError::VersionError => Ok(InodeInfo::Extended(ExtendedInodeInfo {
                    i_format: Format(u16::from_le_bytes(value[0..2].try_into().unwrap())),
                    i_xattr_icount: u16::from_le_bytes(value[2..4].try_into().unwrap()),
                    i_mode: u16::from_le_bytes(value[4..6].try_into().unwrap()),
                    i_reserved: value[6..8].try_into().unwrap(),
                    i_size: u64::from_le_bytes(value[8..16].try_into().unwrap()),
                    i_u: value[16..20].try_into().unwrap(),
                    i_ino: u32::from_le_bytes(value[20..24].try_into().unwrap()),
                    i_uid: u32::from_le_bytes(value[24..28].try_into().unwrap()),
                    i_gid: u32::from_le_bytes(value[28..32].try_into().unwrap()),
                    i_mtime: u64::from_le_bytes(value[32..40].try_into().unwrap()),
                    i_mtime_nsec: u32::from_le_bytes(value[40..44].try_into().unwrap()),
                    i_nlink: u32::from_le_bytes(value[44..48].try_into().unwrap()),
                    i_reserved2: value[48..64].try_into().unwrap(),
                })),
                InodeError::PosixError(e) => Err(e),
            },
        }
    }
}

/// Represents the inode collection which is a hashmap of inodes.
pub(crate) trait InodeCollection {
    type I: Inode + Sized;

    fn iget(&mut self, nid: Nid, filesystem: &dyn FileSystem<Self::I>)
        -> PosixResult<&mut Self::I>;
}

#[cfg(test)]
pub(crate) mod tests {

    extern crate std;
    use super::*;
    use std::collections::{hash_map::Entry, HashMap};

    #[test]
    fn test_inode_size() {
        assert_eq!(core::mem::size_of::<CompactInodeInfo>(), 32);
        assert_eq!(core::mem::size_of::<ExtendedInodeInfo>(), 64);
    }

    pub(crate) struct SimpleInode {
        info: InodeInfo,
        xattr_shared_entries: XAttrSharedEntries,
        nid: Nid,
    }

    impl Inode for SimpleInode {
        fn new(
            _sb: &SuperBlock,
            info: InodeInfo,
            nid: Nid,
            xattr_header: XAttrSharedEntries,
        ) -> Self {
            Self {
                info,
                xattr_shared_entries: xattr_header,
                nid,
            }
        }
        fn xattrs_shared_entries(&self) -> &XAttrSharedEntries {
            &self.xattr_shared_entries
        }
        fn nid(&self) -> Nid {
            self.nid
        }
        fn info(&self) -> &InodeInfo {
            &self.info
        }
    }

    impl InodeCollection for HashMap<Nid, SimpleInode> {
        type I = SimpleInode;
        fn iget(&mut self, nid: Nid, f: &dyn FileSystem<Self::I>) -> PosixResult<&mut Self::I> {
            match self.entry(nid) {
                Entry::Vacant(v) => {
                    let info = f.read_inode_info(nid)?;
                    let xattrs_header = f.read_inode_xattrs_shared_entries(nid, &info)?;
                    Ok(v.insert(Self::I::new(f.superblock(), info, nid, xattrs_header)))
                }
                Entry::Occupied(o) => Ok(o.into_mut()),
            }
        }
    }
}
