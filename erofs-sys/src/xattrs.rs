// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

use super::alloc_helper::*;
use super::data::*;
use super::*;

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

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum XAttrError {
    NotFound,
    NotMatched,
    Invalid,
}
pub(crate) type XAttrResult = Result<Option<Vec<u8>>, XAttrError>;

/// An iterator to read xattrs by comparing the entry's name one by one and reads its value
/// correspondingly.
pub(crate) trait XAttrEntriesProvider {
    fn get_entry_header(&mut self) -> XAttrEntryHeader;
    fn get_xattr_key(
        &mut self,
        pfs: &[XAttrInfix],
        header: &XAttrEntryHeader,
        buffer: &mut [u8],
    ) -> usize;
    fn query_xattr_value(
        &mut self,
        pfs: &[XAttrInfix],
        header: &XAttrEntryHeader,
        name: &[u8],
        index: u32,
        buffer: &mut Option<&mut [u8]>,
    ) -> XAttrResult;
    fn skip_xattr_value(&mut self, header: &XAttrEntryHeader);
}
impl<'a> XAttrEntriesProvider for SkippableContinousIter<'a> {
    fn get_entry_header(&mut self) -> XAttrEntryHeader {
        let mut buf: [u8; 4] = [0; 4];
        self.read(&mut buf);
        XAttrEntryHeader::from(buf)
    }

    fn get_xattr_key(
        &mut self,
        ifs: &[XAttrInfix],
        header: &XAttrEntryHeader,
        buffer: &mut [u8],
    ) -> usize {
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

        self.read(&mut buffer[cur..cur + header.suffix_len as usize]);
        cur += header.suffix_len as usize;
        buffer[cur] = b'\0';
        cur + 1
    }

    fn query_xattr_value(
        &mut self,
        ifs: &[XAttrInfix],
        header: &XAttrEntryHeader,
        name: &[u8],
        index: u32,
        buffer: &mut Option<&mut [u8]>,
    ) -> XAttrResult {
        let xattr_size = round!(
            UP,
            header.suffix_len as Off + header.value_len as Off,
            size_of::<XAttrEntryHeader>() as Off
        );

        let cur = if header.name_index.is_long() {
            let if_index: usize = header.name_index.into();

            if if_index >= ifs.len() {
                return Err(XAttrError::Invalid);
            }

            let infix = ifs.get(if_index).unwrap();
            let ilen = infix.name().len();

            let pf_index = infix.prefix_index();

            if pf_index >= EROFS_XATTRS_PREFIXS.len() as u8 {
                return Err(XAttrError::Invalid);
            }

            let prefix = EROFS_XATTRS_PREFIXS[pf_index as usize];

            let plen = prefix.len();

            if index != pf_index as u32
                || name.len() != plen + ilen + header.suffix_len as usize
                || name[..plen] != *prefix
                || name[plen..plen + ilen] != *infix.name()
            {
                return Err(XAttrError::NotMatched);
            }

            plen + ilen
        } else {
            let pf_index: usize = header.name_index.into();
            if pf_index >= EROFS_XATTRS_PREFIXS.len() {
                return Err(XAttrError::Invalid);
            }

            let prefix = EROFS_XATTRS_PREFIXS[pf_index];
            let plen = prefix.len();

            if pf_index != index as usize
                || plen + header.suffix_len as usize != name.len()
                || name[..plen] != *prefix
            {
                return Err(XAttrError::NotMatched);
            }

            plen
        };

        match self.try_cmp(&name[cur..]) {
            Ok(()) => match buffer.as_mut() {
                Some(b) => {
                    self.read(&mut b[..header.value_len as usize]);
                    Ok(None)
                }
                None => {
                    let mut b: Vec<u8> = vec_with_capacity(header.value_len as usize);
                    self.read(&mut b);
                    Ok(Some(b))
                }
            },
            Err(nvalue) => {
                self.skip(xattr_size - nvalue as Off);
                Err(XAttrError::NotMatched)
            }
        }
    }
    fn skip_xattr_value(&mut self, header: &XAttrEntryHeader) {
        self.skip(
            round!(
                UP,
                header.suffix_len as Off + header.value_len as Off,
                size_of::<XAttrEntryHeader>() as Off
            ) - header.suffix_len as Off,
        );
    }
}
