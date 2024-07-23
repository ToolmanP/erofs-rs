// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use super::*;

// Memory Mapped Device/File so we need to have some external lifetime on the backend trait.
// Note that we do not want the lifetime to infect the MemFileSystem which may have a impact on
// the content iter below. Just use HRTB to dodge the borrow checker.

pub struct MemFileSystem<T>
where
    T: for<'a> MemoryBackend<'a>,
{
    backend: T,
    sb: SuperBlock,
}

impl<T> FileSystem for MemFileSystem<T>
where
    T: for<'a> MemoryBackend<'a>,
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
        Box::new(RefIter::new(&self.backend, MapIter::new(self, inode)))
    }
}

impl<T> MemFileSystem<T>
where
    T: for<'a> MemoryBackend<'a>,
{
    pub fn new(backend: T) -> Self {
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
    use crate::data::MemBuffer;
    use crate::superblock::tests::*;
    use crate::superblock::uncompressed::*;
    use crate::Off;
    use memmap2::MmapMut;

    use crate::superblock::MemorySource;

    // Impl MmapMut to simulate a in-memory image/filesystem
    impl Source for MmapMut {
        fn fill(&self, data: &mut [u8], offset: Off) -> SourceResult<()> {
            self.as_buf(offset, data.len() as u64).map(|buf| {
                data.clone_from_slice(buf.content());
            })
        }
    }

    impl<'a> MemorySource<'a> for MmapMut {
        fn as_buf(&'a self, offset: crate::Off, len: crate::Off) -> SourceResult<MemBuffer<'a>> {
            if offset + len >= self.len() as u64 {
                Err(SourceError::Dummy)
            } else {
                Ok(MemBuffer::new(
                    &self[offset as usize..(offset + len) as usize],
                ))
            }
        }
        fn as_buf_mut(&'a mut self, offset: Off, len: Off) -> SourceResult<MemBufferMut<'a>> {
            if offset + len >= self.len() as u64 {
                Err(SourceError::Dummy)
            } else {
                Ok(MemBufferMut::new(
                    &mut self[offset as usize..(offset + len) as usize],
                    |_| {},
                ))
            }
        }
    }

    #[test]
    fn test_uncompressed_mmap_filesystem() {
        let file = load_fixture();
        let filesystem: Box<dyn FileSystem> =
            Box::new(MemFileSystem::new(UncompressedBackend::new(unsafe {
                MmapMut::map_mut(&file).unwrap()
            })));
        test_superblock_def(filesystem.as_ref());
        test_filesystem_ilookup(filesystem.as_ref());
    }
}
