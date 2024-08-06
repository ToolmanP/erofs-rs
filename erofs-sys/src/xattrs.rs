// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use super::data::*;
use super::*;

use alloc::vec::Vec;

#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct DiskEntryIndexHeader {
    pub(crate) name_filter: u32,
    pub(crate) shared_count: u8,
    pub(crate) reserved: [u8; 7],
}

impl From<[u8; 12]> for DiskEntryIndexHeader {
    fn from(value: [u8; 12]) -> Self {
        unsafe { core::mem::transmute(value) }
    }
}

pub(crate) const XATTRS_HEADER_SIZE: u64 = core::mem::size_of::<DiskEntryIndexHeader>() as u64;

pub(crate) struct MemEntryIndexHeader {
    pub(crate) name_filter: u32,
    pub(crate) shared_indexes: Vec<u32>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct EntryHeader {
    pub(crate) name_len: u8,
    pub(crate) name_index: u8,
    pub(crate) value_len: u16,
}

impl From<[u8; 4]> for EntryHeader {
    fn from(value: [u8; 4]) -> Self {
        unsafe { core::mem::transmute(value) }
    }
}

pub(crate) struct Prefix(pub(crate) Vec<u8>);

impl Prefix {
    fn index(&self) -> u8 {
        self.0[0]
    }
    fn name(&self) -> &[u8] {
        &self.0[1..]
    }
}

pub(crate) trait XAttrsEntryProvider {
    fn get_entry_header(&mut self) -> EntryHeader;
    fn get_xattr_name(&mut self, pfs: &[Prefix], header: &EntryHeader, buffer: &mut [u8]) -> usize;
    fn get_xattr_value(
        &mut self,
        pfs: &[Prefix],
        header: &EntryHeader,
        name: &[u8],
        index: u32,
        buffer: &mut [u8],
    ) -> bool;
    fn fill_xattr_value(&mut self, data: &mut [u8]);
}

pub(crate) const EROFS_XATTR_LONG_PREFIX: u8 = 0x80;
pub(crate) const EROFS_XATTR_LONG_MASK: u8 = 0x7f;

#[allow(unused_macros)]
macro_rules! static_cstr {
    ($l:expr) => {
        unsafe { ::core::ffi::CStr::from_bytes_with_nul_unchecked(concat!($l, "\0").as_bytes()) }
    };
}

pub(crate) const EROFS_XATTRS_PREFIXS: [(&str, usize); 7] = [
    ("", 0),
    ("user.", 5),
    ("system.posix_acl_access", 24),
    ("system.posix_acl_default", 25),
    ("trusted.", 8),
    ("", 0),
    ("security.", 9),
];

impl<'a> XAttrsEntryProvider for SkippableContinousIter<'a> {
    fn get_entry_header(&mut self) -> EntryHeader {
        let mut buf: [u8; 4] = [0; 4];
        self.read(&mut buf);
        EntryHeader::from(buf)
    }

    fn get_xattr_name(&mut self, pfs: &[Prefix], header: &EntryHeader, buffer: &mut [u8]) -> usize {
        let n_len = if header.name_index & EROFS_XATTR_LONG_PREFIX != 0 {
            let pf = pfs
                .get((header.name_index & EROFS_XATTR_LONG_MASK) as usize)
                .unwrap();
            let pf_index = pf.index();
            let (prefix, p_len) = EROFS_XATTRS_PREFIXS[pf_index as usize];
            buffer[..p_len].copy_from_slice(&prefix.as_bytes()[..p_len]);
            buffer[p_len..pf.name().len() + p_len].copy_from_slice(pf.name());
            p_len + pf.name().len()
        } else {
            let (prefix, p_len) = EROFS_XATTRS_PREFIXS[header.name_index as usize];
            buffer[..p_len].copy_from_slice(&prefix.as_bytes()[..p_len]);
            p_len
        };
        self.read(&mut buffer[n_len..n_len + header.name_len as usize]);
        n_len + header.name_len as usize
    }
    fn get_xattr_value(
        &mut self,
        pfs: &[Prefix],
        header: &EntryHeader,
        name: &[u8],
        index: u32,
        buffer: &mut [u8],
    ) -> bool {
        let n_len = name.len();
        let skip_off = header.name_len as Off + header.value_len as Off;
        let n_off = if header.name_index & EROFS_XATTR_LONG_PREFIX != 0 {
            let infix_index = (header.name_index & EROFS_XATTR_LONG_MASK) as usize;
            if infix_index >= pfs.len() {
                return false;
            }

            let pf = pfs.get(infix_index).unwrap();

            let infix_len = pf.name().len();

            if index != pf.index() as u32 || n_len != infix_len + header.name_len as usize {
                return false;
            }
            if name[..infix_len] != *pf.name() {
                return false;
            }

            infix_len
        } else {
            if header.name_index as u32 != index || header.name_len as usize != name.len() {
                return false;
            }
            0
        };
        match self.try_cmp(&name[n_off..]) {
            Ok(()) => {
                self.fill_xattr_value(&mut buffer[..header.value_len as usize]);
                true
            }
            Err(off) => {
                self.skip(skip_off - off as Off);
                false
            }
        }
    }

    fn fill_xattr_value(&mut self, data: &mut [u8]) {
        self.read(data);
    }
}

#[cfg(test)]
pub(crate) mod tests {}
