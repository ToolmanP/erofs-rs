// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use self::operations::get_xattr_prefixes;

use super::*;

pub(crate) struct RawFileSystem<B>
// Only support standard file/device io. Not a continguous region of memory.
where
    B: FileBackend,
{
    backend: B,
    prefixes: Vec<xattrs::Prefix>,
    sb: SuperBlock,
    device_info: DeviceInfo,
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

    fn mapped_iter<'b, 'a: 'b>(&'a self, inode: &'b I) -> Box<dyn BufferMapIter<'a> + 'b> {
        heap_alloc(TempBufferMapIter::new(
            &self.backend,
            MapIter::new(self, inode),
        ))
    }
    fn continous_iter<'a>(
        &'a self,
        offset: Off,
        len: Off,
    ) -> Box<dyn ContinousBufferIter<'a> + 'a> {
        heap_alloc(ContinuousTempBufferIter::new(&self.backend, offset, len))
    }
    fn xattr_prefixes(&self) -> &Vec<xattrs::Prefix> {
        &self.prefixes
    }
    fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }
}

impl<T> RawFileSystem<T>
where
    T: FileBackend,
{
    pub(crate) fn new(backend: T) -> Self {
        let mut buf = SUPERBLOCK_EMPTY_BUF;
        backend.fill(&mut buf, EROFS_SUPER_OFFSET).unwrap();
        let sb: SuperBlock = buf.into();
        let prefixes = get_xattr_prefixes(&sb, &backend);

        let device_info = get_device_infos(&mut ContinuousTempBufferIter::new(
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
    extern crate alloc;
    extern crate std;
    use super::*;
    use crate::inode::tests::*;
    use crate::superblock::tests::*;
    use crate::superblock::uncompressed::*;
    use alloc::boxed::Box;
    use std::collections::HashMap;
    use std::fs::File;
    use std::os::unix::fs::FileExt;

    impl Source for File {
        fn fill(&self, data: &mut [u8], offset: Off) -> SourceResult<u64> {
            self.read_at(data, offset)
                .map_or(Err(SourceError::Dummy), |size| Ok(size as u64))
        }
    }

    impl FileSource for File {}

    #[test]
    fn test_uncompressed_img_filesystem() {
        let file = load_fixture();
        let mut filesystem: SuperblockInfo<SimpleInode, HashMap<Nid, SimpleInode>> =
            SuperblockInfo::new(
                Box::new(RawFileSystem::new(UncompressedBackend::new(file))),
                HashMap::new(),
            );
        test_superblock_def(&mut filesystem);
        test_filesystem_ilookup(&mut filesystem);
    }
}
