use crate::Nid;
#[repr(C)]
pub struct CompactInode {
    pub i_format: u16,
    pub i_xattr_icount: u16,
    pub i_mode: u16,
    pub i_nlink: u16,
    pub i_size: u32,
    pub i_reserved: [u8; 4],
    pub i_u: [u8; 4],
    pub i_ino: u32,
    pub i_uid: u16,
    pub i_gid: u16,
    pub i_reserved2: [u8; 4],
}

#[repr(C)]
pub struct ExtendedInode {
    pub i_format: u16,
    pub i_xattr_icount: u16,
    pub i_mode: u16,
    pub i_reserved: [u8; 2],
    pub i_size: u64,
    pub i_u: [u8; 4],
    pub i_ino: u32,
    pub i_uid: u32,
    pub i_gid: u32,
    pub i_mtime: u64,
    pub i_mtime_nsec: u32,
    pub i_nlink: u32,
    pub i_reserved2: [u8; 16],
}

pub enum GenericInode {
    Extended(ExtendedInode),
    Compact(CompactInode),
}


impl GenericInode {
    pub fn ino(&self) -> u32 {
        match self {
            Self::Extended(extended) => extended.i_ino,
            Self::Compact(compact) => compact.i_ino,
        }
    }
}

pub struct Inode {
    pub g: GenericInode,
    pub nid: Nid,
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
