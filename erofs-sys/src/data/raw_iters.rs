// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

pub(crate) mod ref_iter;
mod traits;
pub(crate) use traits::*;

pub(crate) use super::*;

/// This is used as a iterator to read the metadata buffer. The metadata buffer is a continous 4
/// bytes aligned collection of integers. This is used primarily when reading an inode's xattrs
/// indexe.
pub(crate) struct MetadataBufferIter<'a> {
    backend: &'a dyn Backend,
    buffer: TempBuffer,
    offset: Off,
    total: usize,
}

impl<'a> MetadataBufferIter<'a> {
    pub(crate) fn new(backend: &'a dyn Backend, offset: Off, total: usize) -> Self {
        Self {
            backend,
            buffer: TempBuffer::empty(),
            offset,
            total,
        }
    }
}

impl<'a> Iterator for MetadataBufferIter<'a> {
    type Item = PosixResult<Vec<u8>>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.total == 0 {
            return None;
        }

        if self.buffer.start == self.buffer.maxsize {
            match self
                .backend
                .get_temp_buffer(self.offset, EROFS_TEMP_BLOCK_SZ)
            {
                Ok(buffer) => {
                    self.buffer = buffer;
                }
                Err(e) => {
                    return Some(Err(e));
                }
            }
            self.offset += self.buffer.maxsize as Off;
        }

        let data = self.buffer.content();
        let size = u16::from_le_bytes([data[0], data[1]]) as usize;
        let mut result: Vec<u8> = Vec::new();
        match extend_from_slice(&mut result, &data[2..size + 2]) {
            Ok(()) => {
                self.buffer.start = round!(UP, self.buffer.start + size + 2, 4);
                self.total -= 1;
                Some(Ok(result))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

/// Represents a skippable continuous buffer iterator. This is used primarily for reading the
/// extended attributes. Since the key-value is flattened out in its original format.
pub(crate) struct SkippableContinousIter<'a> {
    iter: Box<dyn ContinousBufferIter<'a> + 'a>,
    data: Box<dyn Buffer + 'a>,
    cur: Off,
}

fn cmp_with_cursor_move(
    lhs: &[u8],
    rhs: &[u8],
    lhs_cur: &mut Off,
    rhs_cur: &mut Off,
    len: Off,
) -> bool {
    let result = lhs[*lhs_cur as usize..(*lhs_cur + len) as usize]
        == rhs[*rhs_cur as usize..(*rhs_cur + len) as usize];
    *lhs_cur += len;
    *rhs_cur += len;
    result
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SkipCmpError {
    PosixError(Errno),
    NotEqual(Off),
}

impl From<Errno> for SkipCmpError {
    fn from(e: Errno) -> Self {
        SkipCmpError::PosixError(e)
    }
}

impl<'a> SkippableContinousIter<'a> {
    pub(crate) fn try_new(
        mut iter: Box<dyn ContinousBufferIter<'a> + 'a>,
    ) -> PosixResult<Option<Self>> {
        if iter.eof() {
            return Ok(None);
        }
        let data = iter.next().unwrap()?;
        Ok(Some(Self { iter, data, cur: 0 }))
    }
    pub(crate) fn skip(&mut self, offset: Off) -> PosixResult<()> {
        let dlen = self.data.content().len() as Off - self.cur;
        if offset <= dlen {
            self.cur += offset;
        } else {
            self.cur = 0;
            self.iter.advance_off(dlen);
            self.data = self.iter.next().unwrap()?;
        }
        Ok(())
    }

    pub(crate) fn read(&mut self, buf: &mut [u8]) -> PosixResult<()> {
        let mut dlen = self.data.content().len() as Off - self.cur;
        let mut bcur = 0 as Off;
        let blen = buf.len() as Off;

        if dlen != 0 && dlen >= blen {
            buf.clone_from_slice(
                &self.data.content()[self.cur as usize..(self.cur + blen) as usize],
            );
            self.cur += blen;
        } else {
            buf[bcur as usize..(bcur + dlen) as usize]
                .copy_from_slice(&self.data.content()[self.cur as usize..]);
            bcur += dlen;
            while bcur < blen {
                self.cur = 0;
                self.data = self.iter.next().unwrap()?;
                dlen = self.data.content().len() as Off;
                if dlen >= blen - bcur {
                    buf[bcur as usize..]
                        .copy_from_slice(&self.data.content()[..(blen - bcur) as usize]);
                    self.cur = blen - bcur;
                    return Ok(());
                } else {
                    buf[bcur as usize..(bcur + dlen) as usize].copy_from_slice(self.data.content());
                    bcur += dlen;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn try_cmp(&mut self, buf: &[u8]) -> Result<(), SkipCmpError> {
        let dlen = self.data.content().len() as Off - self.cur;
        let blen = buf.len() as Off;
        let mut bcur = 0 as Off;

        if dlen != 0 && dlen >= blen {
            if cmp_with_cursor_move(self.data.content(), buf, &mut self.cur, &mut bcur, blen) {
                Ok(())
            } else {
                Err(SkipCmpError::NotEqual(bcur))
            }
        } else {
            if dlen != 0 {
                let clen = dlen.min(blen);
                if !cmp_with_cursor_move(self.data.content(), buf, &mut self.cur, &mut bcur, clen) {
                    return Err(SkipCmpError::NotEqual(bcur));
                }
            }
            while bcur < blen {
                self.cur = 0;
                self.data = self.iter.next().unwrap()?;
                let dlen = self.data.content().len() as Off;
                let clen = dlen.min(blen - bcur);
                if !cmp_with_cursor_move(self.data.content(), buf, &mut self.cur, &mut bcur, clen) {
                    return Err(SkipCmpError::NotEqual(bcur));
                }
            }

            Ok(())
        }
    }
    pub(crate) fn eof(&self) -> bool {
        self.data.content().len() as Off - self.cur == 0 && self.iter.eof()
    }
}
