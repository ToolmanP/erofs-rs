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
    infixes: Vec<XAttrInfix>,
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
    ) -> PosixResult<Box<dyn BufferMapIter<'a> + 'b>> {
        heap_alloc(TempBufferMapIter::new(
            &self.backend,
            MapIter::new(self, inode, offset),
        ))
        .map(|v| v as Box<dyn BufferMapIter<'a> + 'b>)
    }
    fn continous_iter<'a>(
        &'a self,
        offset: Off,
        len: Off,
    ) -> PosixResult<Box<dyn ContinousBufferIter<'a> + 'a>> {
        heap_alloc(ContinuousTempBufferIter::new(&self.backend, offset, len))
            .map(|v| v as Box<dyn ContinousBufferIter<'a> + 'a>)
    }
    fn xattr_infixes(&self) -> &Vec<XAttrInfix> {
        &self.infixes
    }
    fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }
}

impl<T> ImageFileSystem<T>
where
    T: FileBackend,
{
    pub(crate) fn try_new(backend: T) -> PosixResult<Self> {
        let mut buf = SUPERBLOCK_EMPTY_BUF;
        backend.fill(&mut buf, EROFS_SUPER_OFFSET)?;
        let sb: SuperBlock = buf.into();
        let infixes = get_xattr_infixes(&sb, &backend)?;
        let device_info = get_device_infos(&mut ContinuousTempBufferIter::new(
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
    use super::superblock::tests::*;
    use super::superblock::uncompressed::*;
    use super::*;

    use std::boxed::Box;
    use std::collections::HashMap;
    use std::fs::File;
    use std::os::unix::fs::FileExt;

    impl Source for File {
        fn fill(&self, data: &mut [u8], offset: Off) -> PosixResult<u64> {
            self.read_at(data, offset)
                .map_or(Err(Errno::ERANGE), |size| Ok(size as u64))
        }
    }

    impl FileSource for File {}

    #[test]
    fn test_uncompressed_img_filesystem() {
        for file in load_fixtures() {
            let mut sbi: SimpleBufferedFileSystem = SuperblockInfo::new(
                Box::new(ImageFileSystem::try_new(UncompressedBackend::new(file)).unwrap()),
                HashMap::new(),
                (),
            );
            test_filesystem(&mut sbi);
        }
    }
}
