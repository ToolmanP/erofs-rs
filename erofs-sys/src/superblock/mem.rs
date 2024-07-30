// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

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

    fn mapped_iter<'b, 'a: 'b>(&'a self, inode: &'b I) -> Box<dyn BufferMapIter<'a> + 'b> {
        heap_alloc(RefMapIter::new(&self.backend, MapIter::new(self, inode)))
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
                data.clone_from_slice(buf.content());
                buf.content().len() as u64
            })
        }
    }

    impl<'a> PageSource<'a> for MmapMut {
        fn as_buf(&'a self, offset: crate::Off, len: crate::Off) -> SourceResult<RefBuffer<'a>> {
            let pa = PageAddress::from(offset);
            if pa.pg_off + len > EROFS_BLOCK_SZ {
                Err(SourceError::OutBound)
            } else {
                let rlen = len.min(self.len() as u64 - offset);
                let buf =
                    &self[(pa.page as usize)..self.len().min((pa.page + EROFS_BLOCK_SZ) as usize)];
                Ok(RefBuffer::new(buf, pa.pg_off as usize, rlen as usize))
            }
        }

        fn as_buf_mut(&'a mut self, offset: Off, len: Off) -> SourceResult<RefBufferMut<'a>> {
            let pa = PageAddress::from(offset);
            let maxsize = self.len();
            if pa.pg_off + len > EROFS_BLOCK_SZ {
                Err(SourceError::OutBound)
            } else {
                let rlen = len.min(self.len() as u64 - offset);
                let buf =
                    &mut self[(pa.page as usize)..maxsize.min((pa.page + EROFS_BLOCK_SZ) as usize)];
                Ok(RefBufferMut::new(
                    buf,
                    pa.pg_off as usize,
                    rlen as usize,
                    |_| {},
                ))
            }
        }
    }

    #[test]
    fn test_uncompressed_mmap_filesystem() {
        let file = load_fixture();
        let mut filesystem: SuperblockInfo<SimpleInode, HashMap<Nid, SimpleInode>> =
            SuperblockInfo::new(
                Box::new(MemFileSystem::new(UncompressedBackend::new(unsafe {
                    MmapMut::map_mut(&file).unwrap()
                }))),
                HashMap::new(),
            );
        test_superblock_def(&mut filesystem);
        test_filesystem_ilookup(&mut filesystem);
    }
}
