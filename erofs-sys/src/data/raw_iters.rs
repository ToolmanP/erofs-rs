// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

pub(crate) mod ref_iter;
pub(crate) mod temp_iter;
mod traits;
pub(crate) use traits::*;

pub(crate) use super::*;

/// Represents a skippable continuous buffer iterator. This is used primarily for reading the
/// extended attributes. Since the key-value is flattened out in its original format.
pub(crate) struct SkippableContinuousIter<'a> {
    iter: Box<dyn ContinuousBufferIter<'a> + 'a>,
    data: Box<dyn Buffer + 'a>,
    cur: usize,
}

fn cmp_with_cursor_move(
    lhs: &[u8],
    rhs: &[u8],
    lhs_cur: &mut usize,
    rhs_cur: &mut usize,
    len: usize,
) -> bool {
    let result = lhs[*lhs_cur..(*lhs_cur + len)] == rhs[*rhs_cur..(*rhs_cur + len)];
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

impl<'a> SkippableContinuousIter<'a> {
    pub(crate) fn try_new(
        mut iter: Box<dyn ContinuousBufferIter<'a> + 'a>,
    ) -> PosixResult<Option<Self>> {
        if iter.eof() {
            return Ok(None);
        }
        let data = iter.next().unwrap()?;
        Ok(Some(Self { iter, data, cur: 0 }))
    }
    pub(crate) fn skip(&mut self, offset: Off) -> PosixResult<()> {
        let dlen = self.data.content().len() - self.cur;
        if offset as usize <= dlen {
            self.cur += offset as usize;
        } else {
            self.cur = 0;
            self.iter.advance_off(dlen as Off);
            self.data = self.iter.next().unwrap()?;
        }
        Ok(())
    }

    pub(crate) fn read(&mut self, buf: &mut [u8]) -> PosixResult<()> {
        let mut dlen = self.data.content().len() - self.cur;
        let mut bcur = 0_usize;
        let blen = buf.len();
        if dlen != 0 && dlen >= blen {
            buf.clone_from_slice(&self.data.content()[self.cur..(self.cur + blen)]);
            self.cur += blen;
        } else {
            buf[bcur..(bcur + dlen)].copy_from_slice(&self.data.content()[self.cur..]);
            bcur += dlen;
            while bcur < blen {
                self.cur = 0;
                self.data = self.iter.next().unwrap()?;
                dlen = self.data.content().len();
                if dlen >= blen - bcur {
                    buf[bcur..].copy_from_slice(&self.data.content()[..(blen - bcur)]);
                    self.cur = blen - bcur;
                    return Ok(());
                } else {
                    buf[bcur..(bcur + dlen)].copy_from_slice(self.data.content());
                    bcur += dlen;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn try_cmp(&mut self, buf: &[u8]) -> Result<(), SkipCmpError> {
        let dlen = self.data.content().len() - self.cur;
        let blen = buf.len();
        let mut bcur = 0_usize;

        if dlen != 0 && dlen >= blen {
            if cmp_with_cursor_move(self.data.content(), buf, &mut self.cur, &mut bcur, blen) {
                Ok(())
            } else {
                Err(SkipCmpError::NotEqual(bcur as Off))
            }
        } else {
            if dlen != 0 {
                let clen = dlen.min(blen);
                if !cmp_with_cursor_move(self.data.content(), buf, &mut self.cur, &mut bcur, clen) {
                    return Err(SkipCmpError::NotEqual(bcur as Off));
                }
            }
            while bcur < blen {
                self.cur = 0;
                self.data = self.iter.next().unwrap()?;
                let dlen = self.data.content().len();
                let clen = dlen.min(blen - bcur);
                if !cmp_with_cursor_move(self.data.content(), buf, &mut self.cur, &mut bcur, clen) {
                    return Err(SkipCmpError::NotEqual(bcur as Off));
                }
            }

            Ok(())
        }
    }
    pub(crate) fn eof(&self) -> bool {
        self.data.content().len() - self.cur == 0 && self.iter.eof()
    }
}
