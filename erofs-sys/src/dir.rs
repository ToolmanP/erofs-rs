// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

/// On-disk Directory Descriptor Format for EROFS
/// Documented on [EROFS Directory](https://erofs.docs.kernel.org/en/latest/core_ondisk.html#directories)
#[repr(C, packed)]
pub(crate) struct DirentDesc {
    pub(crate) nid: u64,
    pub(crate) nameoff: u16,
    pub(crate) file_type: u8,
    pub(crate) reserved: u8,
}

/// In memory representation of a real directory entry.
pub(crate) struct Dirent<'a> {
    pub(crate) desc: &'a DirentDesc,
    pub(crate) name: &'a [u8],
}

/// Create a collection of directory entries from a buffer.
/// This is a helper struct to iterate over directory entries.
pub(crate) struct DirCollection<'a> {
    data: &'a [u8],
    offset: usize,
    total: usize,
}

impl<'a> DirCollection<'a> {
    pub(crate) fn new(buffer: &'a [u8]) -> Self {
        let desc: &DirentDesc = unsafe { &*(buffer.as_ptr() as *const DirentDesc) };
        Self {
            data: buffer,
            offset: 0,
            total: desc.nameoff as usize / core::mem::size_of::<DirentDesc>(),
        }
    }
    pub(crate) fn dirent(&self, index: usize) -> Option<Dirent<'a>> {
        //SAFETY: Note that DirentDesc is yet another ffi-safe type and the size of Block is larger
        //than that of DirentDesc. It's safe to allow this unsafe cast.
        let descs: &'a [DirentDesc] = unsafe {
            core::slice::from_raw_parts(self.data.as_ptr() as *const DirentDesc, self.total)
        };
        if index >= self.total {
            None
        } else if index == self.total - 1 {
            let len = self.data.len() - descs[self.total - 1].nameoff as usize;
            Some(Dirent {
                desc: &descs[index],
                name: &self.data
                    [descs[index].nameoff as usize..(descs[index].nameoff as usize) + len],
            })
        } else {
            let len = (descs[index + 1].nameoff - descs[index].nameoff) as usize;
            Some(Dirent {
                desc: &descs[index],
                name: &self.data
                    [descs[index].nameoff as usize..(descs[index].nameoff as usize) + len],
            })
        }
    }
    pub(crate) fn skip_dir(&mut self, offset: usize) {
        self.offset += offset;
    }
    pub(crate) fn total(&self) -> usize {
        self.total
    }
}

impl<'a> Iterator for DirCollection<'a> {
    type Item = Dirent<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.dirent(self.offset).map(|x| {
            self.offset += 1;
            x
        })
    }
}

impl<'a> Dirent<'a> {
    pub(crate) fn dirname(&self) -> &'a [u8] {
        self.name
    }
}
