// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-or-later

use super::alloc_helper::*;
use super::data::raw_iters::*;
use super::errnos::*;
use super::*;
use crate::round;

use alloc::vec::Vec;
use core::mem::size_of;

/// The header of the xattr entry index.
/// This is used to describe the superblock's xattrs collection.
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct XAttrSharedEntrySummary {
    pub(crate) name_filter: u32,
    pub(crate) shared_count: u8,
    pub(crate) reserved: [u8; 7],
}

impl From<[u8; 12]> for XAttrSharedEntrySummary {
    fn from(value: [u8; 12]) -> Self {
        Self {
            name_filter: u32::from_le_bytes(value[0..4].try_into().unwrap()),
            shared_count: value[4],
            reserved: value[5..12].try_into().unwrap(),
        }
    }
}

pub(crate) const XATTR_ENTRY_SUMMARY_BUF: [u8; 12] = [0u8; 12];

/// Represented as a inmemory memory entry index header used by SuperBlockInfo.
pub(crate) struct XAttrSharedEntries {
    pub(crate) name_filter: u32,
    pub(crate) shared_indexes: Vec<u32>,
}

/// Represents the name index for infixes or prefixes.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct XattrNameIndex(u8);

impl core::cmp::PartialEq<u8> for XattrNameIndex {
    fn eq(&self, other: &u8) -> bool {
        if self.0 & EROFS_XATTR_LONG_PREFIX != 0 {
            self.0 & EROFS_XATTR_LONG_MASK == *other
        } else {
            self.0 == *other
        }
    }
}

impl XattrNameIndex {
    pub(crate) fn is_long(&self) -> bool {
        self.0 & EROFS_XATTR_LONG_PREFIX != 0
    }
}

impl From<u8> for XattrNameIndex {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

#[allow(clippy::from_over_into)]
impl Into<usize> for XattrNameIndex {
    fn into(self) -> usize {
        if self.0 & EROFS_XATTR_LONG_PREFIX != 0 {
            (self.0 & EROFS_XATTR_LONG_MASK) as usize
        } else {
            self.0 as usize
        }
    }
}

/// This is on-disk representation of xattrs entry header.
/// This is used to describe one extended attribute.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct XAttrEntryHeader {
    pub(crate) suffix_len: u8,
    pub(crate) name_index: XattrNameIndex,
    pub(crate) value_len: u16,
}

impl From<[u8; 4]> for XAttrEntryHeader {
    fn from(value: [u8; 4]) -> Self {
        Self {
            suffix_len: value[0],
            name_index: value[1].into(),
            value_len: u16::from_le_bytes(value[2..4].try_into().unwrap()),
        }
    }
}

/// Xattr Common Infix holds the prefix index in the first byte and all the common infix data in
/// the rest of the bytes.
pub(crate) struct XAttrInfix(pub(crate) Vec<u8>);

impl XAttrInfix {
    fn prefix_index(&self) -> u8 {
        self.0[0]
    }
    fn name(&self) -> &[u8] {
        &self.0[1..]
    }
}

pub(crate) const EROFS_XATTR_LONG_PREFIX: u8 = 0x80;
pub(crate) const EROFS_XATTR_LONG_MASK: u8 = EROFS_XATTR_LONG_PREFIX - 1;

/// Supported xattr prefixes
pub(crate) const EROFS_XATTRS_PREFIXS: [&[u8]; 7] = [
    b"",
    b"user.",
    b"system.posix_acl_access",
    b"system.posix_acl_default",
    b"trusted.",
    b"",
    b"security.",
];

/// Represents the value of an xattr entry or the size of it if the buffer is present in the query.
#[derive(Debug)]
pub(crate) enum XAttrValue {
    Buffer(usize),
    Vec(Vec<u8>),
}

/// An iterator to read xattrs by comparing the entry's name one by one and reads its value
/// correspondingly.
pub(crate) trait XAttrEntriesProvider {
    fn get_entry_header(&mut self) -> PosixResult<XAttrEntryHeader>;
    fn get_xattr_key(
        &mut self,
        pfs: &[XAttrInfix],
        header: &XAttrEntryHeader,
        buffer: &mut [u8],
    ) -> PosixResult<usize>;
    fn query_xattr_value(
        &mut self,
        pfs: &[XAttrInfix],
        header: &XAttrEntryHeader,
        name: &[u8],
        index: u32,
        buffer: &mut Option<&mut [u8]>,
    ) -> PosixResult<XAttrValue>;
    fn skip_xattr_value(&mut self, header: &XAttrEntryHeader) -> PosixResult<()>;
}
impl<'a> XAttrEntriesProvider for SkippableContinuousIter<'a> {
    fn get_entry_header(&mut self) -> PosixResult<XAttrEntryHeader> {
        let mut buf: [u8; 4] = [0; 4];
        self.read(&mut buf).map(|_| XAttrEntryHeader::from(buf))
    }

    fn get_xattr_key(
        &mut self,
        ifs: &[XAttrInfix],
        header: &XAttrEntryHeader,
        buffer: &mut [u8],
    ) -> PosixResult<usize> {
        let mut cur = if header.name_index.is_long() {
            let if_index: usize = header.name_index.into();
            let infix: &XAttrInfix = ifs.get(if_index).unwrap();

            let pf_index = infix.prefix_index();
            let prefix = EROFS_XATTRS_PREFIXS[pf_index as usize];
            let plen = prefix.len();

            buffer[..plen].copy_from_slice(&prefix[..plen]);
            buffer[plen..infix.name().len() + plen].copy_from_slice(infix.name());

            plen + infix.name().len()
        } else {
            let pf_index: usize = header.name_index.into();
            let prefix = EROFS_XATTRS_PREFIXS[pf_index];
            let plen = prefix.len();
            buffer[..plen].copy_from_slice(&prefix[..plen]);
            plen
        };

        self.read(&mut buffer[cur..cur + header.suffix_len as usize])?;
        cur += header.suffix_len as usize;
        buffer[cur] = b'\0';
        Ok(cur + 1)
    }

    fn query_xattr_value(
        &mut self,
        ifs: &[XAttrInfix],
        header: &XAttrEntryHeader,
        name: &[u8],
        index: u32,
        buffer: &mut Option<&mut [u8]>,
    ) -> PosixResult<XAttrValue> {
        let xattr_size = round!(
            UP,
            header.suffix_len as Off + header.value_len as Off,
            size_of::<XAttrEntryHeader>() as Off
        );

        let cur = if header.name_index.is_long() {
            let if_index: usize = header.name_index.into();

            if if_index >= ifs.len() {
                return Err(ENODATA);
            }

            let infix = ifs.get(if_index).unwrap();
            let ilen = infix.name().len();

            let pf_index = infix.prefix_index();

            if pf_index >= EROFS_XATTRS_PREFIXS.len() as u8 {
                return Err(ENODATA);
            }

            if index != pf_index as u32
                || name.len() != ilen + header.suffix_len as usize
                || name[..ilen] != *infix.name()
            {
                return Err(ENODATA);
            }
            ilen
        } else {
            let pf_index: usize = header.name_index.into();
            if pf_index >= EROFS_XATTRS_PREFIXS.len() {
                return Err(ENODATA);
            }

            if pf_index != index as usize || header.suffix_len as usize != name.len() {
                return Err(ENODATA);
            }
            0
        };

        match self.try_cmp(&name[cur..]) {
            Ok(()) => match buffer.as_mut() {
                Some(b) => {
                    if b.len() < header.value_len as usize {
                        return Err(ERANGE);
                    }
                    self.read(&mut b[..header.value_len as usize])?;
                    Ok(XAttrValue::Buffer(header.value_len as usize))
                }
                None => {
                    let mut b: Vec<u8> = vec_with_capacity(header.value_len as usize)?;
                    self.read(&mut b)?;
                    Ok(XAttrValue::Vec(b))
                }
            },
            Err(skip_err) => match skip_err {
                SkipCmpError::NotEqual(nvalue) => {
                    self.skip(xattr_size - nvalue)?;
                    Err(ENODATA)
                }
                SkipCmpError::PosixError(e) => Err(e),
            },
        }
    }
    fn skip_xattr_value(&mut self, header: &XAttrEntryHeader) -> PosixResult<()> {
        self.skip(
            round!(
                UP,
                header.suffix_len as Off + header.value_len as Off,
                size_of::<XAttrEntryHeader>() as Off
            ) - header.suffix_len as Off,
        )
    }
}
