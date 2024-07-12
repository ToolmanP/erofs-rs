use super::*;
use crate::*;

pub(crate) struct UncompressedBackend<T>
where
    T: Source,
{
    source: T,
}

impl<T> Backend for UncompressedBackend<T>
where
    T: Source,
{
    fn fill(&self, data: &mut [u8], offset: Off) -> BackendResult<()> {
        self.source
            .fill(data, offset)
            .map_err(|_| BackendError::Dummy)
    }
    fn get_block(&self, offset: Off) -> BackendResult<Block> {
        match self.source.get_block(offset) {
            Ok(block) => Ok(block),
            Err(_) => Err(BackendError::Dummy),
        }
    }
}

impl<T> FileBackend for UncompressedBackend<T> where T: FileSource {}

impl<'a, T> MemoryBackend<'a> for UncompressedBackend<T>
where
    T: MemorySource<'a>,
{
    fn as_ref_block(&'a self, offset: Off) -> BackendResult<&'a Block> {
        self.source
            .as_ref_block(offset)
            .map_err(|_| BackendError::Dummy)
    }
}

impl<T: Source> UncompressedBackend<T> {
    pub(crate) fn new(source: T) -> Self {
        Self { source }
    }
}

impl<T> From<T> for UncompressedBackend<T>
where
    T: Source,
{
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
