// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use alloc::vec::Vec;

use super::alloc_helper::*;
use super::data::*;
use super::inode::*;
use super::superblock::*;
use super::xattrs;
use super::*;

// Because of the brain dead features of borrow-checker, it cannot statically analyze which part of the struct is exclusively borrowed.
// Refactor out the real file operations, so that we can make sure things will get compiled.

pub(crate) fn read_inode<'a, I, C>(
    filesystem: &'a dyn FileSystem<I>,
    collection: &'a mut C,
    nid: Nid,
) -> &'a mut I
where
    I: Inode,
    C: InodeCollection<I = I>,
{
    collection.iget(nid, filesystem)
}

pub(crate) fn ilookup<'a, I, C>(
    filesystem: &'a dyn FileSystem<I>,
    collection: &'a mut C,
    name: &str,
) -> Option<&'a mut I>
where
    I: Inode,
    C: InodeCollection<I = I>,
{
    let mut nid = filesystem.superblock().root_nid as Nid;
    for part in name.split('/') {
        if part.is_empty() {
            continue;
        }
        let inode = read_inode(filesystem, collection, nid); // this part collection is reborrowed for shorter
                                                             // lifetime inside the loop;
        nid = filesystem.find_nid(inode, part)?
    }
    Some(read_inode(filesystem, collection, nid))
}

pub(crate) fn dir_lookup<'a, I, C>(
    filesystem: &'a dyn FileSystem<I>,
    collection: &'a mut C,
    inode: &I,
    name: &str,
) -> Option<&'a mut I>
where
    I: Inode,
    C: InodeCollection<I = I>,
{
    let mut nid = inode.nid();
    for part in name.split('/') {
        if part.is_empty() {
            continue;
        }
        let inode = read_inode(filesystem, collection, nid); // this part collection is reborrowed for shorter
                                                             // lifetime inside the loop;
        nid = filesystem.find_nid(inode, part)?
    }
    Some(read_inode(filesystem, collection, nid))
}

pub(crate) fn get_xattr_prefixes(sb: &SuperBlock, backend: &dyn Backend) -> Vec<xattrs::Prefix> {
    let mut result: Vec<xattrs::Prefix> = Vec::new();
    for data in MetadataBufferIter::new(
        backend,
        (sb.xattr_prefix_start << 2) as Off,
        sb.xattr_prefix_count as usize,
    ) {
        push_vec(&mut result, xattrs::Prefix(data));
    }
    result
}
