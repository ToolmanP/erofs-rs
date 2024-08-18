// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

use self::operations::get_xattr_prefixes;

use super::*;

// Memory Mapped Device/File so we need to have some external lifetime on the backend trait.
// Note that we do not want the lifetime to infect the MemFileSystem which may have a impact on
// the content iter below. Just use HRTB to dodge the borrow checker.

pub(crate) struct MemFileSystem<T>
where
    T: for<'a> MemoryBackend<'a>,
{
    backend: T,
    sb: SuperBlock,
    prefixes: Vec<xattrs::Prefix>,
    device_info: DeviceInfo,
}

impl<I, T> FileSystem<I> for MemFileSystem<T>
where
    T: for<'a> MemoryBackend<'a>,
    I: Inode,
{
    fn superblock(&self) -> &SuperBlock {
        &self.sb
    }
    fn backend(&self) -> &dyn Backend {
        &self.backend
    }

    fn mapped_iter<'b, 'a: 'b>(
        &'a self,
        inode: &'b I,
        offset: Off,
    ) -> Box<dyn BufferMapIter<'a> + 'b> {
        heap_alloc(RefMapIter::new(
            &self.backend,
            MapIter::new(self, inode, offset),
        ))
    }
    fn continous_iter<'a>(
        &'a self,
        offset: Off,
        len: Off,
    ) -> Box<dyn ContinousBufferIter<'a> + 'a> {
        heap_alloc(ContinuousRefIter::new(&self.backend, offset, len))
    }
    fn xattr_prefixes(&self) -> &Vec<xattrs::Prefix> {
        &self.prefixes
    }
    fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }
}

impl<T> MemFileSystem<T>
where
    T: for<'a> MemoryBackend<'a>,
{
    pub(crate) fn new(backend: T) -> Self {
        let mut buf = SUPERBLOCK_EMPTY_BUF;
        backend.fill(&mut buf, EROFS_SUPER_OFFSET).unwrap();
        let sb: SuperBlock = buf.into();
        let prefixes = get_xattr_prefixes(&sb, &backend);
        let device_info = get_device_infos(&mut ContinuousRefIter::new(
            &backend,
            sb.devt_slotoff as Off * 128,
            sb.extra_devices as Off * 128,
        ));
        Self {
            backend,
            sb,
            prefixes,
            device_info,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::data::RefBuffer;
    use crate::inode::tests::*;
    use crate::superblock::tests::*;
    use crate::superblock::uncompressed::*;
    use crate::superblock::PageSource;
    use crate::Off;
    use memmap2::MmapMut;
    use std::collections::HashMap;

    // Impl MmapMut to simulate a in-memory image/filesystem
    impl Source for MmapMut {
        fn fill(&self, data: &mut [u8], offset: Off) -> SourceResult<u64> {
            self.as_buf(offset, data.len() as u64).map(|buf| {
                let len = buf.content().len();
                data[..len].clone_from_slice(buf.content());
                len as Off
            })
        }
    }

    impl<'a> PageSource<'a> for MmapMut {
        fn as_buf(&'a self, offset: crate::Off, len: crate::Off) -> SourceResult<RefBuffer<'a>> {
            let accessor = TempBlockAccessor::from(offset);
            let maxsize = self.len();
            let rlen = len.min(self.len() as u64 - offset);
            let buf = &self[(accessor.base as usize)
                ..maxsize.min((accessor.base + EROFS_TEMP_BLOCK_SZ) as usize)];

            Ok(RefBuffer::new(
                buf,
                accessor.off as usize,
                rlen as usize,
                |_| {},
            ))
        }

        fn as_buf_mut(&'a mut self, offset: Off, len: Off) -> SourceResult<RefBufferMut<'a>> {
            let accessor = TempBlockAccessor::from(offset);
            let maxsize = self.len();
            let rlen = len.min(self.len() as u64 - offset);
            let buf = &mut self[(accessor.base as usize)
                ..maxsize.min((accessor.base + EROFS_TEMP_BLOCK_SZ) as usize)];
            Ok(RefBufferMut::new(
                buf,
                accessor.off as usize,
                rlen as usize,
                |_| {},
            ))
        }
    }

    #[test]
    fn test_uncompressed_mmap_filesystem() {
        for file in load_fixtures() {
            let mut sbi: SuperblockInfo<SimpleInode, HashMap<Nid, SimpleInode>> =
                SuperblockInfo::new(
                    Box::new(MemFileSystem::new(UncompressedBackend::new(unsafe {
                        MmapMut::map_mut(&file).unwrap()
                    }))),
                    HashMap::new(),
                );
            test_filesystem(&mut sbi);
        }
    }
}
