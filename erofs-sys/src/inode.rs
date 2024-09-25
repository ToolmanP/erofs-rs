// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-or-later

use super::superblock::*;
use super::xattrs::*;
use super::*;
use core::ffi::*;
use core::mem::size_of;

/// Represents the compact bitfield of the Erofs Inode format.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Format(pub(crate) u16);

pub(crate) const INODE_VERSION_MASK: u16 = 0x1;
pub(crate) const INODE_VERSION_BIT: u16 = 0;

pub(crate) const INODE_LAYOUT_BIT: u16 = 1;
pub(crate) const INODE_LAYOUT_MASK: u16 = 0x7;

/// Helper macro to extract property from the bitfield.
macro_rules! extract {
    ($name: expr, $bit: expr, $mask: expr) => {
        ($name >> $bit) & ($mask)
    };
}

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
pub enum Layout {
    /// Flat Plain
    FlatPlain,
    /// CompressedFull
    CompressedFull,
    /// FlatInline
    FlatInline,
    /// CompressedCompact
    CompressedCompact,
    /// Chunk
    Chunk,
    /// Unknown
    Unknown,
}

///.Inode Type
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Type {
    /// Regular
    Regular,
    /// Directory
    Directory,
    /// Link
    Link,
    /// Character
    Character,
    /// Block
    Block,
    /// Fifo
    Fifo,
    /// Socket
    Socket,
    /// Unknown
    Unknown,
}

/// This is format extracted from i_format bit representation.
/// This includes various infos and specs about the inode.
impl Format {
    pub(crate) fn version(&self) -> Version {
        match extract!(self.0, INODE_VERSION_BIT, INODE_VERSION_MASK) {
            0 => Version::Compat,
            1 => Version::Extended,
            _ => Version::Unknown,
        }
    }

    /// layout
    pub fn layout(&self) -> Layout {
        match extract!(self.0, INODE_LAYOUT_BIT, INODE_LAYOUT_MASK) {
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
pub struct CompactInodeInfo {
    pub(crate) i_format: Format,
    pub(crate) i_xattr_icount: u16,
    /// i_mode
    pub i_mode: u16,
    /// i_nlink
    pub i_nlink: u16,
    /// i_size
    pub i_size: u32,
    pub(crate) i_reserved: [u8; 4],
    pub(crate) i_u: [u8; 4],
    /// i_ino
    pub i_ino: u32,
    /// i_uid
    pub i_uid: u16,
    /// i_gid
    pub i_gid: u16,
    pub(crate) i_reserved2: [u8; 4],
}

/// Represents the extended inode which resides on-disk.
/// This is documented in https://erofs.docs.kernel.org/en/latest/core_ondisk.html#inodes
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ExtendedInodeInfo {
    pub(crate) i_format: Format,
    pub(crate) i_xattr_icount: u16,
    /// i_mode
    pub i_mode: u16,
    pub(crate) i_reserved: [u8; 2],
    /// i_size
    pub i_size: u64,
    pub(crate) i_u: [u8; 4],
    /// i_ino
    pub i_ino: u32,
    /// i_uid
    pub i_uid: u32,
    /// i_gid
    pub i_gid: u32,
    /// i_mtime
    pub i_mtime: u64,
    /// m_time_nsec
    pub i_mtime_nsec: u32,
    /// n_link
    pub i_nlink: u32,
    pub(crate) i_reserved2: [u8; 16],
}

/// Represents the inode info which is either compact or extended.
#[derive(Clone, Copy)]
pub enum InodeInfo {
    /// Extended
    Extended(ExtendedInodeInfo),
    /// Compact
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

/// Represents the inode spec which is either data or device.
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub(crate) enum Spec {
    Chunk(ChunkFormat),
    RawBlk(u32),
    Device(u32),
    CompressedBlocks(u32),
    Unknown,
}

/// Convert the spec from the format of the inode based on the layout.
impl From<(&[u8; 4], Layout)> for Spec {
    fn from(value: (&[u8; 4], Layout)) -> Self {
        match value.1 {
            Layout::FlatInline | Layout::FlatPlain => Spec::RawBlk(u32::from_le_bytes(*value.0)),
            Layout::CompressedFull | Layout::CompressedCompact => {
                Spec::CompressedBlocks(u32::from_le_bytes(*value.0))
            }
            Layout::Chunk => Self::Chunk(ChunkFormat(u16::from_le_bytes([value.0[0], value.0[1]]))),
            // We don't support compressed inlines or compressed chunks currently.
            _ => Spec::Unknown,
        }
    }
}

/// Helper functions for Inode Info.
impl InodeInfo {
    const S_IFMT: u16 = 0o170000;
    const S_IFSOCK: u16 = 0o140000;
    const S_IFLNK: u16 = 0o120000;
    const S_IFREG: u16 = 0o100000;
    const S_IFBLK: u16 = 0o60000;
    const S_IFDIR: u16 = 0o40000;
    const S_IFCHR: u16 = 0o20000;
    const S_IFIFO: u16 = 0o10000;
    const S_ISUID: u16 = 0o4000;
    const S_ISGID: u16 = 0o2000;
    const S_ISVTX: u16 = 0o1000;
    pub(crate) fn ino(&self) -> u32 {
        match self {
            Self::Extended(extended) => extended.i_ino,
            Self::Compact(compact) => compact.i_ino,
        }
    }

    /// format
    pub fn format(&self) -> Format {
        match self {
            Self::Extended(extended) => extended.i_format,
            Self::Compact(compact) => compact.i_format,
        }
    }

    /// file_size
    pub fn file_size(&self) -> Off {
        match self {
            Self::Extended(extended) => extended.i_size,
            Self::Compact(compact) => compact.i_size as u64,
        }
    }

    /// inode_size
    pub fn inode_size(&self) -> Off {
        match self {
            Self::Extended(_) => 64,
            Self::Compact(_) => 32,
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
            0o40000 | 0o100000 | 0o120000 => Spec::from((u, self.format().layout())),
            // We don't support device inodes currently.
            _ => Spec::Unknown,
        }
    }

    /// i_node type
    pub fn inode_type(&self) -> Type {
        let mode = match self {
            Self::Extended(extended) => extended.i_mode,
            Self::Compact(compact) => compact.i_mode,
        };
        match mode & Self::S_IFMT {
            Self::S_IFDIR => Type::Directory, // Directory
            Self::S_IFREG => Type::Regular,   // Regular File
            Self::S_IFLNK => Type::Link,      // Symbolic Link
            Self::S_IFIFO => Type::Fifo,      // FIFO
            Self::S_IFSOCK => Type::Socket,   // Socket
            Self::S_IFBLK => Type::Block,     // Block
            Self::S_IFCHR => Type::Character, // Character
            _ => Type::Unknown,
        }
    }

    /// inode_perm
    pub fn inode_perm(&self) -> u16 {
        let mode = match self {
            Self::Extended(extended) => extended.i_mode,
            Self::Compact(compact) => compact.i_mode,
        };
        mode & 0o777
    }

    pub(crate) fn xattr_size(&self) -> Off {
        match self {
            Self::Extended(extended) => {
                size_of::<XAttrSharedEntrySummary>() as Off
                    + (size_of::<c_int>() as Off) * (extended.i_xattr_icount as Off - 1)
            }
            Self::Compact(_) => 0,
        }
    }

    pub(crate) fn xattr_count(&self) -> u16 {
        match self {
            Self::Extended(extended) => extended.i_xattr_icount,
            Self::Compact(compact) => compact.i_xattr_icount,
        }
    }
}

pub(crate) type CompactInodeInfoBuf = [u8; size_of::<CompactInodeInfo>()];
pub(crate) type ExtendedInodeInfoBuf = [u8; size_of::<ExtendedInodeInfo>()];
pub(crate) const DEFAULT_INODE_BUF: ExtendedInodeInfoBuf = [0; size_of::<ExtendedInodeInfo>()];

/// The inode trait which represents the inode in the filesystem.
pub trait Inode: Sized {
    /// New Inode
    fn new(
        _sb: &SuperBlock,
        info: InodeInfo,
        nid: Nid,
        xattrs_shared_entries: XAttrSharedEntries,
    ) -> Self;
    /// Info
    fn info(&self) -> &InodeInfo;
    /// SharedEntries
    fn xattrs_shared_entries(&self) -> &XAttrSharedEntries;
    /// Nid
    fn nid(&self) -> Nid;
}

/// Represents the error which occurs when trying to convert the inode.
#[derive(Debug)]
pub enum InodeError {
    /// Version Error
    VersionError,
    /// Posix Error
    PosixError(Errno),
}

impl TryFrom<CompactInodeInfoBuf> for CompactInodeInfo {
    type Error = InodeError;
    fn try_from(value: CompactInodeInfoBuf) -> Result<Self, Self::Error> {
        let inode: CompactInodeInfo = Self {
            i_format: Format(u16::from_le_bytes([value[0], value[1]])),
            i_xattr_icount: u16::from_le_bytes([value[2], value[3]]),
            i_mode: u16::from_le_bytes([value[4], value[5]]),
            i_nlink: u16::from_le_bytes([value[6], value[7]]),
            i_size: u32::from_le_bytes([value[8], value[9], value[10], value[11]]),
            i_reserved: value[12..16].try_into().unwrap(),
            i_u: value[16..20].try_into().unwrap(),
            i_ino: u32::from_le_bytes([value[20], value[21], value[22], value[23]]),
            i_uid: u16::from_le_bytes([value[24], value[25]]),
            i_gid: u16::from_le_bytes([value[26], value[27]]),
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

impl<I> TryFrom<(&dyn FileSystem<I>, Nid)> for InodeInfo
where
    I: Inode,
{
    type Error = Errno;
    fn try_from(value: (&dyn FileSystem<I>, Nid)) -> Result<Self, Self::Error> {
        let f = value.0;
        let sb = f.superblock();
        let nid = value.1;
        let offset = sb.iloc(nid);
        let accessor = sb.blk_access(offset);
        let mut buf: ExtendedInodeInfoBuf = DEFAULT_INODE_BUF;
        f.backend().fill(&mut buf[0..32], 0, offset)?;
        let compact_buf: CompactInodeInfoBuf = buf[0..32].try_into().unwrap();
        let r: Result<CompactInodeInfo, InodeError> = CompactInodeInfo::try_from(compact_buf);
        match r {
            Ok(compact) => Ok(InodeInfo::Compact(compact)),
            Err(e) => match e {
                InodeError::VersionError => {
                    let gotten = (sb.blksz() - accessor.off + 32).min(64);
                    f.backend().fill(
                        &mut buf[32..(32 + gotten).min(64) as usize],
                        0,
                        offset + 32,
                    )?;

                    if gotten < 32 {
                        f.backend().fill(
                            &mut buf[(32 + gotten) as usize..64],
                            0,
                            sb.blkpos(sb.blknr(offset) + 1),
                        )?;
                    }
                    Ok(InodeInfo::Extended(ExtendedInodeInfo {
                        i_format: Format(u16::from_le_bytes([buf[0], buf[1]])),
                        i_xattr_icount: u16::from_le_bytes([buf[2], buf[3]]),
                        i_mode: u16::from_le_bytes([buf[4], buf[5]]),
                        i_reserved: buf[6..8].try_into().unwrap(),
                        i_size: u64::from_le_bytes([
                            buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
                        ]),
                        i_u: buf[16..20].try_into().unwrap(),
                        i_ino: u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
                        i_uid: u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]),
                        i_gid: u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]),
                        i_mtime: u64::from_le_bytes([
                            buf[32], buf[33], buf[34], buf[35], buf[36], buf[37], buf[38], buf[39],
                        ]),
                        i_mtime_nsec: u32::from_le_bytes([buf[40], buf[41], buf[42], buf[43]]),
                        i_nlink: u32::from_le_bytes([buf[44], buf[45], buf[46], buf[47]]),
                        i_reserved2: buf[48..64].try_into().unwrap(),
                    }))
                }
                InodeError::PosixError(e) => Err(e),
            },
        }
    }
}

/// Represents the inode collection which is a hashmap of inodes.
pub trait InodeCollection {
    /// Inode Assocaited Types
    type I: Inode + Sized;

    /// get the inode based on nid and filesystem
    fn iget(&mut self, nid: Nid, filesystem: &dyn FileSystem<Self::I>)
        -> PosixResult<&mut Self::I>;
    /// release inode
    fn release(&mut self, nid: Nid);
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
        fn release(&mut self, nid: Nid) {
            self.remove_entry(&nid);
        }
    }
}
