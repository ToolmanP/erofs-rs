// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

use super::data::raw_iters::ref_iter::*;
use super::operations::*;
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
    infixes: Vec<XAttrInfix>,
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

    fn as_filesystem(&self) -> &dyn FileSystem<I> {
        self
    }

    fn mapped_iter<'b, 'a: 'b>(
        &'a self,
        inode: &'b I,
        offset: Off,
    ) -> PosixResult<Box<dyn BufferMapIter<'a> + 'b>> {
        heap_alloc(RefMapIter::new(
            &self.sb,
            &self.backend,
            MapIter::new(self, inode, offset),
        ))
        .map(|v| v as Box<dyn BufferMapIter<'a> + 'b>)
    }
    fn continuous_iter<'a>(
        &'a self,
        offset: Off,
        len: Off,
    ) -> PosixResult<Box<dyn ContinuousBufferIter<'a> + 'a>> {
        heap_alloc(ContinuousRefIter::new(&self.sb, &self.backend, offset, len))
            .map(|v| v as Box<dyn ContinuousBufferIter<'a> + 'a>)
    }
    fn xattr_infixes(&self) -> &Vec<XAttrInfix> {
        &self.infixes
    }
    fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }
}

impl<T> MemFileSystem<T>
where
    T: for<'a> MemoryBackend<'a>,
{
    pub(crate) fn try_new(backend: T) -> PosixResult<Self> {
        let mut buf = SUPERBLOCK_EMPTY_BUF;
        backend.fill(&mut buf, EROFS_SUPER_OFFSET)?;
        let sb: SuperBlock = buf.into();
        let infixes = get_xattr_infixes(&mut ContinuousRefIter::new(
            &sb,
            &backend,
            sb.xattr_prefix_start as Off,
            sb.xattr_prefix_count as Off * 4,
        ))?;
        let device_info = get_device_infos(&mut ContinuousRefIter::new(
            &sb,
            &backend,
            sb.devt_slotoff as Off * 128,
            sb.extra_devices as Off * 128,
        ))?;
        Ok(Self {
            backend,
            sb,
            infixes,
            device_info,
        })
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::super::*;
    use super::superblock::backends::uncompressed::*;
    use super::superblock::tests::*;
    use super::*;

    use memmap2::MmapMut;
    use std::collections::HashMap;

    // Impl MmapMut to simulate a in-memory image/filesystem
    impl Source for MmapMut {
        fn fill(&self, data: &mut [u8], offset: Off) -> PosixResult<u64> {
            self.as_buf(offset, data.len() as u64).map(|buf| {
                let len = buf.len();
                data[..len].clone_from_slice(buf);
                len as Off
            })
        }
    }

    impl<'a> PageSource<'a> for MmapMut {
        fn as_buf(&'a self, offset: Off, len: Off) -> PosixResult<&'a [u8]> {
            let maxsize = self.len() as Off;
            let rlen = len.min(self.len() as u64 - offset);
            let buf = &self[offset as usize..maxsize.min(offset + rlen) as usize];
            Ok(buf)
        }
    }

    #[test]
    fn test_uncompressed_mmap_filesystem() {
        for file in load_fixtures() {
            let mut sbi: SimpleBufferedFileSystem = SuperblockInfo::new(
                Box::new(
                    MemFileSystem::try_new(UncompressedBackend::new(unsafe {
                        MmapMut::map_mut(&file).unwrap()
                    }))
                    .unwrap(),
                ),
                HashMap::new(),
                (),
            );
            test_filesystem(&mut sbi);
        }
    }
}
