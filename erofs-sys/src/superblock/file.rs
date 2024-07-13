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
        TempBufferIter::new(&self.backend, MapIter::new(self, inode)).find_nid(name)
    }
}

impl<T> RawFileSystem<T>
where
    T: FileBackend,
{
    pub(crate) fn new(backend: T) -> Self {
        let mut buf = SUPERBLOCK_EMPTY_BUF;
        backend.fill(&mut buf, EROFS_SUPER_OFFSET).unwrap();
        Self {
            backend,
            sb: buf.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::fs::File;
    use std::os::unix::fs::FileExt;

    use crate::data::uncompressed::UncompressedBackend;
    use crate::superblock::tests::load_fixture;

    pub(crate) const SB_MAGIC: u32 = 0xE0F5E1E2;

    impl Source for File {
        fn fill(&self, data: &mut [u8], offset: Off) -> SourceResult<()> {
            self.read_exact_at(data, offset)
                .map_err(|_| SourceError::Dummy)
        }
        fn get_block(&self, offset: Off) -> SourceResult<Block> {
            let mut block: Block = EROFS_EMPTY_BLOCK;
            self.fill(&mut block, round!(DOWN, offset, EROFS_BLOCK_SZ as Off))
                .map(|()| block)
        }
    }

    impl FileSource for File {}

    fn get_uncompressed_filesystem() -> RawFileSystem<UncompressedBackend<File>> {
        let file = load_fixture();
        RawFileSystem::new(UncompressedBackend::new(file))
    }

    #[test]
    fn test_superblock_def() {
        let filesystem = get_uncompressed_filesystem();
        assert_eq!(filesystem.superblock().magic, SB_MAGIC);
    }

    #[test]
    fn test_filesystem_ilookup() {
        let filesystem = get_uncompressed_filesystem();
        let inode = filesystem.ilookup("/texts/lipsum.txt").unwrap();
        assert_eq!(inode.nid, 640);
    }
}
