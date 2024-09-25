// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-or-later

// Because of the brain dead features of borrow-checker, it cannot statically analyze which part of the struct is exclusively borrowed.
// Refactor out the real file operations, so that we can make sure things will get compiled.

use alloc::vec::Vec;

use super::alloc_helper::*;
use super::data::raw_iters::*;
use super::errnos::*;
use super::inode::*;
use super::superblock::*;
use super::xattrs::*;
use super::*;

use crate::round;

/// Read inode from inode collection.
pub fn read_inode<'a, I, C>(
    filesystem: &'a dyn FileSystem<I>,
    collection: &'a mut C,
    nid: Nid,
) -> PosixResult<&'a mut I>
where
    I: Inode,
    C: InodeCollection<I = I>,
{
    collection.iget(nid, filesystem)
}

/// Lookup
pub fn lookup<'a, I, C>(
    filesystem: &'a dyn FileSystem<I>,
    collection: &'a mut C,
    mut nid: Nid,
    name: &str,
) -> PosixResult<&'a mut I>
where
    I: Inode,
    C: InodeCollection<I = I>,
{
    for part in name.split('/') {
        if part.is_empty() {
            continue;
        }
        let inode = read_inode(filesystem, collection, nid)?; // this part collection is reborrowed for shorter
                                                              // lifetime inside the loop;
        match filesystem.find_nid(inode, part)? {
            Some(n) => {
                nid = n;
            }
            None => {
                return Err(ENOENT);
            }
        }
    }
    read_inode(filesystem, collection, nid)
}

/// dir_lookup
pub fn dir_lookup<'a, I, C>(
    filesystem: &'a dyn FileSystem<I>,
    collection: &'a mut C,
    inode: &I,
    name: &str,
) -> PosixResult<&'a mut I>
where
    I: Inode,
    C: InodeCollection<I = I>,
{
    filesystem
        .find_nid(inode, name)?
        .map_or(Err(ENOENT), |nid| read_inode(filesystem, collection, nid))
}

/// get_xattr
pub fn get_xattr<I>(
    filesystem: &dyn FileSystem<I>,
    inode: &I,
    index: u32,
    name: &[u8],
    buffer: &mut Option<&mut [u8]>,
) -> PosixResult<XAttrValue>
where
    I: Inode,
{
    filesystem.get_xattr(inode, index, name, buffer)
}

/// list_xattr
pub fn list_xattrs<I, C>(
    filesystem: &dyn FileSystem<I>,
    inode: &I,
    buffer: &mut [u8],
) -> PosixResult<usize>
where
    I: Inode,
{
    filesystem.list_xattrs(inode, buffer)
}

pub(crate) fn get_xattr_infixes<'a>(
    iter: &mut (dyn ContinuousBufferIter<'a> + 'a),
) -> PosixResult<Vec<XAttrInfix>> {
    let mut result: Vec<XAttrInfix> = Vec::new();
    for data in iter {
        let buffer = data?;
        let buf = buffer.content();
        let len = buf.len();
        let mut cur: usize = 0;
        while cur <= len {
            let mut infix: Vec<u8> = Vec::new();
            let size = u16::from_le_bytes([buf[cur], buf[cur + 1]]) as usize;
            extend_from_slice(&mut infix, &buf[cur + 2..cur + 2 + size])?;
            push_vec(&mut result, XAttrInfix(infix))?;
            cur = round!(UP, cur + 2 + size, 4);
        }
    }
    Ok(result)
}
