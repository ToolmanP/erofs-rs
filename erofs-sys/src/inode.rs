// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use crate::data::Backend;
use crate::superblock::SuperBlockInfo;
use crate::*;

use core::mem::size_of;

pub(crate) struct NameiContext {
    pub(crate) nid: Nid,
    pub(crate) ftype: u32,
}

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
pub(crate) struct CompactInode {
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
pub(crate) struct ExtendedInode {
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
pub(crate) enum GenericInode {
    Extended(ExtendedInode),
    Compact(CompactInode),
}

#[derive(Clone, Copy)]
pub(crate) struct ChunkSpec {
    chunkformat: u16,
    chunkbits: u8,
}

#[derive(Clone, Copy)]
pub(crate) enum Spec {
    Chunk(ChunkSpec),
    Data(u32),
    Device(u32),
    Unknown,
}

impl GenericInode {
    pub fn ino(&self) -> u32 {
        match self {
            Self::Extended(extended) => extended.i_ino,
            Self::Compact(compact) => compact.i_ino,
        }
    }

    pub fn format(&self) -> Format {
        match self {
            Self::Extended(extended) => extended.i_format,
            Self::Compact(compact) => compact.i_format,
        }
    }

    pub fn file_size(&self) -> Off {
        match self {
            Self::Extended(extended) => extended.i_size,
            Self::Compact(compact) => compact.i_size as u64,
        }
    }

    pub fn inode_size(&self) -> Off {
        match self {
            Self::Extended(_) => 64,
            Self::Compact(_) => 32,
        }
    }

    pub fn xattr_size(&self) -> Off {
        match self {
            Self::Extended(extended) => 12 + 4 * (extended.i_xattr_icount as u64 - 1),
            Self::Compact(_) => 0,
        }
    }

    pub fn spec(&self) -> Spec {
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

    pub fn inode_type(&self) -> Type {
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

pub(crate) type InodeBuf = [u8; size_of::<ExtendedInode>()];
pub(crate) const DEFAULT_INODE_BUF: InodeBuf = [0; size_of::<ExtendedInode>()];

#[derive(Clone, Copy)]
pub struct Inode {
    pub inner: GenericInode,
    pub nid: Nid,
}

#[derive(Debug)]
pub enum InodeError {
    VersionError,
    UnknownError,
}

type InodeResult<T> = Result<T, InodeError>;

impl<'a> TryFrom<&'a [u8]> for &'a CompactInode {
    type Error = InodeError;
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        //SAFETY: all the types present are ffi-safe. safe to cast here since only [u8;64] could be
        //passed into this function and it's definitely safe.
        let inode: &'a CompactInode = unsafe { &*(value.as_ptr() as *const CompactInode) };
        let ifmt = &inode.i_format;
        match ifmt.version() {
            Version::Compat => Ok(inode),
            Version::Extended => Err(InodeError::VersionError),
            _ => Err(InodeError::UnknownError),
        }
    }
}

impl TryFrom<InodeBuf> for GenericInode {
    type Error = InodeError;
    fn try_from(value: InodeBuf) -> Result<Self, Self::Error> {
        let r: Result<&CompactInode, Self::Error> = value.as_slice().try_into();
        match r {
            Ok(compact) => Ok(GenericInode::Compact(*compact)),
            Err(e) => match e {
                //SAFETY: Note that try_into will return VersionError. This suggests that current
                //buffer contains the extended inode. Since the types used are FFI-safe, it's safe
                //to transtmute it here.
                InodeError::VersionError => Ok(GenericInode::Extended(unsafe {
                    core::mem::transmute(value)
                })),
                _ => Err(e),
            },
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_inode_size() {
        assert_eq!(core::mem::size_of::<CompactInode>(), 32);
        assert_eq!(core::mem::size_of::<ExtendedInode>(), 64);
    }
}
