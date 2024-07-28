// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use alloc::boxed::Box;

use super::*;
use core::mem::{size_of, MaybeUninit};

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Format(u16);

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) enum Version {
    Compat,
    Extended,
    Unknown,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) enum Layout {
    FlatPlain,
    CompressedFull,
    FlatInline,
    CompressedCompact,
    Chunk,
    Unknown,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Type {
    Regular,
    Directory,
    Link,
    Character,
    Block,
    FIFO,
    Socket,
    Unknown,
}

impl Format {
    pub(crate) fn version(&self) -> Version {
        match (self.0 >> 0) & ((1 << 1) - 1) {
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

#[derive(Clone, Copy)]
pub(crate) enum InodeInfo {
    Extended(ExtendedInodeInfo),
    Compact(CompactInodeInfo),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ChunkSpec {
    chunkformat: u16,
    chunkbits: u8,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Spec {
    Chunk(ChunkSpec),
    Data(u32),
    Device(u32),
    Unknown,
}

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
            0o40000 => Spec::Data(u32::from_le_bytes(*u)), // Directory
            0o100000 => Spec::Data(u32::from_le_bytes(*u)), // Regular File
            0o120000 => Spec::Data(u32::from_le_bytes(*u)), // Symbolic Link
            0o10000 => Spec::Device(0),                    // FIFO
            0o140000 => Spec::Device(0),                   // Socket
            0o60000 => unimplemented!(),                   // Block
            0o20000 => unimplemented!(),                   // Character
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
            0o10000 => Type::FIFO,      // FIFO
            0o140000 => Type::Socket,   // Socket
            0o60000 => Type::Block,     // Block
            0o20000 => Type::Character, // Character
            _ => Type::Unknown,
        }
    }
}

pub(crate) type InodeInfoBuf = [u8; size_of::<ExtendedInodeInfo>()];
pub(crate) const DEFAULT_INODE_BUF: InodeInfoBuf = [0; size_of::<ExtendedInodeInfo>()];

pub trait Inode: Sized {
    fn new(info: InodeInfo, nid: Nid, xattrs_header: xattrs::MemEntryIndexHeader) -> Self;
    fn info(&self) -> &InodeInfo;
    fn xattrs_header(&self) -> &xattrs::MemEntryIndexHeader;
    fn nid(&self) -> &Nid;
}

#[derive(Debug)]
pub enum InodeError {
    VersionError,
    UnknownError,
}

type InodeResult<T> = Result<T, InodeError>;

impl<'a> TryFrom<&'a [u8]> for &'a CompactInodeInfo {
    type Error = InodeError;
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        //SAFETY: all the types present are ffi-safe. safe to cast here since only [u8;64] could be
        //passed into this function and it's definitely safe.
        let inode: &'a CompactInodeInfo = unsafe { &*(value.as_ptr() as *const CompactInodeInfo) };
        let ifmt = &inode.i_format;
        match ifmt.version() {
            Version::Compat => Ok(inode),
            Version::Extended => Err(InodeError::VersionError),
            _ => Err(InodeError::UnknownError),
        }
    }
}

impl TryFrom<InodeInfoBuf> for InodeInfo {
    type Error = InodeError;
    fn try_from(value: InodeInfoBuf) -> Result<Self, Self::Error> {
        let r: Result<&CompactInodeInfo, Self::Error> = value.as_slice().try_into();
        match r {
            Ok(compact) => Ok(InodeInfo::Compact(*compact)),
            Err(e) => match e {
                //SAFETY: Note that try_into will return VersionError. This suggests that current
                //buffer contains the extended inode. Since the types used are FFI-safe, it's safe
                //to transtmute it here.
                InodeError::VersionError => {
                    Ok(InodeInfo::Extended(unsafe { core::mem::transmute(value) }))
                }
                _ => Err(e),
            },
        }
    }
}

pub trait InodeCollection {
    type I: Inode + Sized;

    // Design Pattern:
    // In ilocked_get5 the inode will go through the late init phase;
    // we mimic this correspondingly;
    fn new_uninit_raw() -> Box<MaybeUninit<Self::I>> {
        Box::new(MaybeUninit::<Self::I>::uninit())
    }

    fn iget(&mut self, nid: Nid) -> (&mut MaybeUninit<Self::I>, bool);
}

#[cfg(test)]
pub(crate) mod tests {

    extern crate std;
    use super::*;
    use crate::xattrs;
    use std::collections::{hash_map::Entry, HashMap};

    #[test]
    fn test_inode_size() {
        assert_eq!(core::mem::size_of::<CompactInodeInfo>(), 32);
        assert_eq!(core::mem::size_of::<ExtendedInodeInfo>(), 64);
    }

    pub(crate) struct SimpleInode {
        info: InodeInfo,
        xattr_header: xattrs::MemEntryIndexHeader,
        nid: Nid,
    }

    impl Inode for SimpleInode {
        fn new(info: InodeInfo, nid: Nid, xattr_header: xattrs::MemEntryIndexHeader) -> Self {
            Self {
                info,
                xattr_header,
                nid,
            }
        }
        fn xattrs_header(&self) -> &xattrs::MemEntryIndexHeader {
            &self.xattr_header
        }
        fn nid(&self) -> &Nid {
            &self.nid
        }
        fn info(&self) -> &InodeInfo {
            &self.info
        }
    }

    impl InodeCollection for HashMap<Nid, MaybeUninit<SimpleInode>> {
        type I = SimpleInode;
        fn iget(&mut self, nid: Nid) -> (&mut MaybeUninit<Self::I>, bool) {
            match self.entry(nid) {
                Entry::Vacant(v) => (v.insert(MaybeUninit::<Self::I>::uninit()), false),
                Entry::Occupied(o) => (o.into_mut(), true),
            }
        }
    }
}
