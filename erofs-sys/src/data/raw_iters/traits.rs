use super::super::*;

pub(crate) trait BufferMapIter<'a>:
    Iterator<Item = PosixResult<Box<dyn Buffer + 'a>>>
{
}

/// Represents a basic iterator over a range of bytes from data backends.
/// Note that this is skippable and can be used to move the iterator's cursor forward.
pub(crate) trait ContinousBufferIter<'a>:
    Iterator<Item = PosixResult<Box<dyn Buffer + 'a>>>
{
    fn advance_off(&mut self, offset: Off);
    fn eof(&self) -> bool;
}
