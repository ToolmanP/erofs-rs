// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use crate::inode::Inode;
use crate::inode::InodeCollection;
use crate::superblock::*;
use crate::Nid;

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
    let (inode, is_init) = collection.iget(nid);
    if !is_init {
        inode.write(I::new(filesystem.read_inode_info(nid), nid));
    }
    unsafe { inode.assume_init_mut() }
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
