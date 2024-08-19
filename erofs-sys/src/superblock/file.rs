// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

use self::operations::get_xattr_infixes;

use super::*;

pub(crate) struct ImageFileSystem<B>
// Only support standard file/device io. Not a continguous region of memory.
where
    B: FileBackend,
{
    backend: B,
    prefixes: Vec<XAttrInfix>,
    sb: SuperBlock,
    device_info: DeviceInfo,
}

impl<I, B> FileSystem<I> for ImageFileSystem<B>
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

    fn mapped_iter<'b, 'a: 'b>(
        &'a self,
        inode: &'b I,
        offset: Off,
    ) -> Box<dyn BufferMapIter<'a> + 'b> {
        heap_alloc(TempBufferMapIter::new(
            &self.backend,
            MapIter::new(self, inode, offset),
        ))
    }
    fn continous_iter<'a>(
        &'a self,
        offset: Off,
        len: Off,
    ) -> Box<dyn ContinousBufferIter<'a> + 'a> {
        heap_alloc(ContinuousTempBufferIter::new(&self.backend, offset, len))
    }
    fn xattr_infixes(&self) -> &Vec<XAttrInfix> {
        &self.prefixes
    }
    fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }
}

impl<T> ImageFileSystem<T>
where
    T: FileBackend,
{
    pub(crate) fn new(backend: T) -> Self {
        let mut buf = SUPERBLOCK_EMPTY_BUF;
        backend.fill(&mut buf, EROFS_SUPER_OFFSET).unwrap();
        let sb: SuperBlock = buf.into();
        let prefixes = get_xattr_infixes(&sb, &backend);

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
        for file in load_fixtures() {
            let mut sbi: SuperblockInfo<SimpleInode, HashMap<Nid, SimpleInode>> =
                SuperblockInfo::new(
                    Box::new(ImageFileSystem::new(UncompressedBackend::new(file))),
                    HashMap::new(),
                );
            test_filesystem(&mut sbi);
        }
    }
}
