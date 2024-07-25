// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use super::*;

pub struct RawFileSystem<B>
// Only support standard file/device io. Not a continguous region of memory.
where
    B: FileBackend,
{
    backend: B,
    sb: SuperBlock,
}

impl<I, B> FileSystem<I> for RawFileSystem<B>
where
    B: FileBackend,
    I: Inode,
{
    fn superblock(&self) -> &SuperBlock {
        &self.sb
    }
    fn backend(&self) -> &dyn Backend {
        &self.backend
    }
    fn content_iter<'b, 'a: 'b>(
        &'a self,
        inode: &'b I,
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
    use super::*;
    use crate::inode::tests::*;
    use crate::superblock::tests::*;
    use crate::superblock::uncompressed::*;
    use alloc::boxed::Box;
    use core::mem::MaybeUninit;
    use std::collections::HashMap;
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
        let mut filesystem: SuperblockInfo<SimpleInode, HashMap<Nid, MaybeUninit<SimpleInode>>> =
            SuperblockInfo::new(
                Box::new(RawFileSystem::new(UncompressedBackend::new(file))),
                HashMap::new(),
            );
        test_superblock_def(&mut filesystem);
        let inode = test_filesystem_ilookup(&mut filesystem);
    }
}
