use super::*;
use core::marker::PhantomData;

pub struct MemFileSystem<'a, T>
// Memory Mapped Device/File so we need to have some life
where
    T: MemoryBackend<'a>,
{
    backend: T,
    sb: SuperBlock,
    _marker: PhantomData<dyn MemorySource<'a>>,
}

impl<'a, T> SuperBlockInfo<'a, T> for MemFileSystem<'a, T>
where
    T: MemoryBackend<'a>,
{
    fn superblock(&self) -> &SuperBlock {
        &self.sb
    }
    fn backend(&self) -> &T {
        &self.backend
    }
    fn find_nid(&'a self, inode: &Inode, name: &str) -> Option<Nid> {
        BlockRefIter::new(&self.backend, MapIter::new(self, inode)).find_nid(name)
    }
}
