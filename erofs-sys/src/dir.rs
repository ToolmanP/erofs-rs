use crate::data::*;
use crate::*;

#[repr(C, packed)]
pub(crate) struct DirentDesc {
    pub nid: u64,
    pub nameoff: u16,
    pub file_type: u8,
    pub reserved: u8,
}

pub(crate) struct Dirent<'a> {
    pub desc: &'a DirentDesc,
    pub name: &'a [u8],
}

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
