// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use super::*;

pub struct RawFileSystem<T>
// Only support standard file/device io. Not a continguous region of memory.
where
    T: FileBackend,
{
    backend: T,
    sb: SuperBlock,
}

impl<T> FileSystem for RawFileSystem<T>
where
    T: FileBackend,
{
    fn superblock(&self) -> &SuperBlock {
        &self.sb
    }
    fn backend(&self) -> &dyn Backend {
        &self.backend
    }
    fn content_iter<'b, 'a: 'b>(
        &'a self,
        inode: &'b Inode,
    ) -> Box<dyn Iterator<Item = Box<dyn Buffer + 'b>> + 'b> {
        Box::new(TempBufferIter::new(
            &self.backend,
            MapIter::new(self, inode),
        ))
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
    extern crate alloc;
    extern crate std;
    use alloc::boxed::Box;

    use super::*;
    use crate::data::uncompressed::UncompressedBackend;
    use crate::superblock::tests::*;
    use std::fs::File;
    use std::os::unix::fs::FileExt;

    impl Source for File {
        fn fill(&self, data: &mut [u8], offset: Off) -> SourceResult<()> {
            self.read_exact_at(data, offset)
                .map_err(|_| SourceError::Dummy)
        }
    }

    impl FileSource for File {}

    #[test]
    fn test_uncompressed_img_filesystem() {
        let file = load_fixture();
        let filesystem: Box<dyn FileSystem> = Box::new(RawFileSystem::new(UncompressedBackend::new(file)));
        test_superblock_def(filesystem.as_ref());
        test_filesystem_ilookup(filesystem.as_ref());
    }
}
