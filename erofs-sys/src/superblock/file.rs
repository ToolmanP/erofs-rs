use super::*;

pub struct RawFileSystem<T>
// Only support standard file/device io. Not a continguous region of memory.
where
    T: FileBackend,
{
    backend: T,
    sb: SuperBlock,
}

impl<'a, T> SuperBlockInfo<'a, T> for RawFileSystem<T>
where
    T: FileBackend,
{
    fn superblock(&self) -> &SuperBlock {
        &self.sb
    }
    fn backend(&self) -> &T {
        &self.backend
    }
    fn find_nid(&'a self, inode: &Inode, name: &str) -> Option<Nid> {
        BlockIter::new(&self.backend, MapIter::new(self, inode)).find_nid(name)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
}
