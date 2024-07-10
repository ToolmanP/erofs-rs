use super::*;
use crate::*;

pub(crate) struct UncompressedBackend<'a, T>
where
    T: Source,
{
    source: &'a T,
}

impl<'a, T> Backend for UncompressedBackend<'a, T>
where
    T: Source,
{
    fn fill(&self, data: &mut [u8], offset: Off, len: Off) -> BackendResult<Off> {
        self.source
            .fill(data, offset, len)
            .map_err(|_| BackendError::Dummy)
    }
    fn get_block(&self, offset: Off) -> BackendResult<Block> {
        let real_offset = offset & (!(EROFS_BLOCK_SZ - 1) as u64);
        let mut block = EROFS_EMPTY_BLOCK;
        match self
            .source
            .fill(&mut block, real_offset, EROFS_BLOCK_SZ as u64)
        {
            Ok(_) => Ok(block),
            Err(_) => Err(BackendError::Dummy),
        }
    }
}

impl<'a, T> FileBackend for UncompressedBackend<'a, T> where T: FileSource {}

impl<'a, T> MemoryBackend<'a> for UncompressedBackend<'a, T>
where
    T: MemorySource<'a>,
{
    fn as_ref_block(&'a self, offset: Off) -> BackendResult<&'a Block> {
        self.source
            .as_ref_block(offset)
            .map_err(|_| BackendError::Dummy)
    }
}
