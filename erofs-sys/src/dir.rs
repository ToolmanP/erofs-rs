use crate::Offset;
use crate::inode::Inode;

#[repr(C, packed)]
pub struct Dirent {
    pub nid: u32,
    pub nameoff: u16,
    pub file_type: u8,
    pub reserved: u8,
}

impl<'a> From<&'a Inode> for InodeContext<'a> {
    fn from(value: &'a Inode) -> Self {
        Self {
            inode: value,
            pos: 0
        }
    }
}

pub struct InodeContext<'a> {
    inode: &'a Inode, 
    pos: Offset
}

impl<'a> Iterator for InodeContext<'a> {
    type Item = Dirent;
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

