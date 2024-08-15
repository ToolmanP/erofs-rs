// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::round;

use super::alloc_helper::*;
use super::data::*;
use super::devices::*;
use super::dir::*;
use super::inode::*;
use super::map::*;
use super::xattrs;
use super::xattrs::*;
use super::*;

use core::mem::size_of;

pub(crate) mod file;
pub(crate) mod mem;

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub(crate) struct SuperBlock {
    pub(crate) magic: u32,
    pub(crate) checksum: i32,
    pub(crate) feature_compat: i32,
    pub(crate) blkszbits: u8,
    pub(crate) sb_extslots: u8,
    pub(crate) root_nid: i16,
    pub(crate) inos: i64,
    pub(crate) build_time: i64,
    pub(crate) build_time_nsec: i32,
    pub(crate) blocks: i32,
    pub(crate) meta_blkaddr: u32,
    pub(crate) xattr_blkaddr: u32,
    pub(crate) uuid: [u8; 16],
    pub(crate) volume_name: [u8; 16],
    pub(crate) feature_incompat: i32,
    pub(crate) compression: i16,
    pub(crate) extra_devices: i16,
    pub(crate) devt_slotoff: i16,
    pub(crate) dirblkbits: u8,
    pub(crate) xattr_prefix_count: u8,
    pub(crate) xattr_prefix_start: i32,
    pub(crate) packed_nid: i64,
    pub(crate) xattr_filter_reserved: u8,
    pub(crate) reserved: [u8; 23],
}

// SAFETY: SuperBlock uses all ffi-safe types.
impl TryFrom<&[u8]> for SuperBlock {
    type Error = core::array::TryFromSliceError;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        value[0..128].try_into()
    }
}

// SAFETY: SuperBlock uses all ffi-safe types.
impl From<[u8; 128]> for SuperBlock {
    fn from(value: [u8; 128]) -> Self {
        Self {
            magic: u32::from_le_bytes(value[0..4].try_into().unwrap()),
            checksum: i32::from_le_bytes(value[4..8].try_into().unwrap()),
            feature_compat: i32::from_le_bytes(value[8..12].try_into().unwrap()),
            blkszbits: value[12],
            sb_extslots: value[13],
            root_nid: i16::from_le_bytes(value[14..16].try_into().unwrap()),
            inos: i64::from_le_bytes(value[16..24].try_into().unwrap()),
            build_time: i64::from_le_bytes(value[24..32].try_into().unwrap()),
            build_time_nsec: i32::from_le_bytes(value[32..36].try_into().unwrap()),
            blocks: i32::from_le_bytes(value[36..40].try_into().unwrap()),
            meta_blkaddr: u32::from_le_bytes(value[40..44].try_into().unwrap()),
            xattr_blkaddr: u32::from_le_bytes(value[44..48].try_into().unwrap()),
            uuid: value[48..64].try_into().unwrap(),
            volume_name: value[64..80].try_into().unwrap(),
            feature_incompat: i32::from_le_bytes(value[80..84].try_into().unwrap()),
            compression: i16::from_le_bytes(value[84..86].try_into().unwrap()),
            extra_devices: i16::from_le_bytes(value[86..88].try_into().unwrap()),
            devt_slotoff: i16::from_le_bytes(value[88..90].try_into().unwrap()),
            dirblkbits: value[90],
            xattr_prefix_count: value[91],
            xattr_prefix_start: i32::from_le_bytes(value[92..96].try_into().unwrap()),
            packed_nid: i64::from_le_bytes(value[96..104].try_into().unwrap()),
            xattr_filter_reserved: value[104],
            reserved: value[105..128].try_into().unwrap(),
        }
    }
}

pub(crate) type SuperBlockBuf = [u8; size_of::<SuperBlock>()];
pub(crate) const SUPERBLOCK_EMPTY_BUF: SuperBlockBuf = [0; size_of::<SuperBlock>()];

/// Used for external ondisk block buffer address calculation.
pub(crate) struct BlockAccessor {
    pub(crate) base: Off,
    pub(crate) blk_index: Off,
    pub(crate) blk_off: Off,
    pub(crate) blk_len: Off,
}

impl BlockAccessor {
    pub(crate) fn new(sb: &SuperBlock, address: Off) -> Self {
        let bits = sb.blkszbits as Off;
        let sz = 1 << bits;
        let mask = sz - 1;
        BlockAccessor {
            base: (address >> bits) << bits,
            blk_index: address >> bits,
            blk_off: address & mask,
            blk_len: sz - (address & mask),
        }
    }
}

pub(crate) trait FileSystem<I>
where
    I: Inode,
{
    fn superblock(&self) -> &SuperBlock;
    fn device_info(&self) -> &DeviceInfo;
    fn backend(&self) -> &dyn Backend;
    fn blknr(&self, pos: Off) -> Blk {
        (pos >> self.superblock().blkszbits) as Blk
    }

    fn blkpos(&self, blk: Blk) -> Off {
        (blk as Off) << self.superblock().blkszbits
    }

    fn blkoff(&self, offset: Off) -> Off {
        offset & (self.blksz() - 1)
    }

    fn blksz(&self) -> Off {
        1 << self.superblock().blkszbits
    }

    fn blk_round_up(&self, addr: Off) -> Blk {
        ((addr + self.blksz() - 1) >> self.superblock().blkszbits) as Blk
    }

    fn iloc(&self, nid: Nid) -> Off {
        let sb = &self.superblock();
        self.blkpos(sb.meta_blkaddr) + ((nid as Off) << (5 as Off))
    }

    fn read_inode_info(&self, nid: Nid) -> InodeInfo {
        let offset = self.iloc(nid);
        let mut buf: ExtendedInodeInfoBuf = DEFAULT_INODE_BUF;
        self.backend().fill(&mut buf, offset).unwrap();
        InodeInfo::try_from(buf).unwrap()
    }

    fn xattr_prefixes(&self) -> &Vec<xattrs::Prefix>;

    // Currently we eagerly initialized all xattrs;
    //
    fn read_inode_xattrs_index(&self, nid: Nid) -> xattrs::MemEntryIndexHeader {
        let offset = self.iloc(nid);
        let pa = PageAccessor::from(offset);

        let mut buf = EROFS_PAGE;
        let mut indexes: Vec<u32> = Vec::new();

        let rlen = self
            .backend()
            .fill(&mut buf[0..pa.pg_len as usize], offset)
            .unwrap();

        let header: xattrs::DiskEntryIndexHeader =
            unsafe { *(buf.as_ptr() as *const xattrs::DiskEntryIndexHeader) };
        let inline_count =
            (((rlen - xattrs::XATTRS_HEADER_SIZE) >> 2) as usize).min(header.shared_count as usize);
        let outbound_count = header.shared_count as usize - inline_count;

        extend_from_slice(&mut indexes, unsafe {
            core::slice::from_raw_parts(
                (buf[xattrs::XATTRS_HEADER_SIZE as usize..pa.pg_len as usize])
                    .as_ptr()
                    .cast(),
                inline_count,
            )
        });

        if outbound_count == 0 {
            xattrs::MemEntryIndexHeader {
                name_filter: header.name_filter,
                shared_indexes: indexes,
            }
        } else {
            for block in self.continous_iter(
                round!(UP, offset, EROFS_PAGE_SZ),
                (outbound_count << 2) as Off,
            ) {
                let data = block.content();
                extend_from_slice(&mut indexes, unsafe {
                    core::slice::from_raw_parts(data.as_ptr().cast(), data.len() >> 2)
                });
            }
            xattrs::MemEntryIndexHeader {
                name_filter: header.name_filter,
                shared_indexes: indexes,
            }
        }
    }
    fn flatmap(&self, inode: &I, offset: Off, inline: bool) -> MapResult {
        let nblocks = self.blk_round_up(inode.info().file_size());

        let blkaddr = match inode.info().spec() {
            Spec::Data(ds) => match ds {
                DataSpec::RawBlk(blkaddr) => blkaddr,
                _ => unimplemented!(),
            },
            _ => unimplemented!(),
        };

        let lastblk = if inline { nblocks - 1 } else { nblocks };

        let len = inode.info().file_size() - offset;
        if offset < self.blkpos(lastblk) {
            Ok(Map {
                logical: Segment { start: offset, len },
                physical: Segment {
                    start: self.blkpos(blkaddr) + offset,
                    len,
                },
                algorithm_format: 0,
                device_id: 0,
                map_type: MapType::Normal,
            })
        } else if inline {
            Ok(Map {
                logical: Segment { start: offset, len },
                physical: Segment {
                    start: self.iloc(inode.nid())
                        + inode.info().inode_size()
                        + inode.info().xattr_size()
                        + self.blkoff(offset),
                    len,
                },
                algorithm_format: 0,
                device_id: 0,
                map_type: MapType::Meta,
            })
        } else {
            Err(MapError::OutofBound)
        }
    }

    fn chunk_map(&self, inode: &I, offset: Off) -> MapResult {
        let cs = match inode.info().spec() {
            Spec::Data(ds) => match ds {
                DataSpec::ChunkFormat(cs) => cs,
                _ => unimplemented!(),
            },
            _ => unimplemented!(),
        };
        let chunkbits = ((cs & CHUNK_BLKBITS_MASK) + self.superblock().blkszbits as u16) as Off;

        let chunknr = offset >> chunkbits;
        if cs & CHUNK_FORMAT_INDEXES != 0 {
            let unit = size_of::<ChunkIndex>() as Off;
            let pos = round!(
                UP,
                self.iloc(inode.nid())
                    + inode.info().inode_size()
                    + inode.info().xattr_size()
                    + unit * chunknr,
                unit
            );
            let mut buf = [0u8; size_of::<ChunkIndex>()];
            self.backend().fill(&mut buf, pos).unwrap();
            let chunk_index = ChunkIndex::from(buf);

            if chunk_index.blkaddr == u32::MAX {
                Err(MapError::OutofBound)
            } else {
                Ok(Map {
                    logical: Segment {
                        start: chunknr << chunkbits,
                        len: 1 << chunkbits,
                    },
                    physical: Segment {
                        start: self.blkpos(chunk_index.blkaddr),
                        len: 1 << chunkbits,
                    },
                    algorithm_format: 0,
                    device_id: chunk_index.device_id & self.device_info().mask,
                    map_type: MapType::Normal,
                })
            }
        } else {
            let unit = 4;
            let pos = round!(
                UP,
                self.iloc(inode.nid())
                    + inode.info().inode_size()
                    + inode.info().xattr_size()
                    + unit * chunknr,
                unit
            );
            let mut buf = [0u8; 4];
            self.backend().fill(&mut buf, pos).unwrap();
            let blkaddr = u32::from_le_bytes(buf);
            if blkaddr == u32::MAX {
                Err(MapError::OutofBound)
            } else {
                Ok(Map {
                    logical: Segment {
                        start: chunknr << chunkbits,
                        len: 1 << chunkbits,
                    },
                    physical: Segment {
                        start: self.blkpos(blkaddr),
                        len: 1 << chunkbits,
                    },
                    algorithm_format: 0,
                    device_id: 0,
                    map_type: MapType::Normal,
                })
            }
        }
    }

    fn map(&self, inode: &I, offset: Off) -> MapResult {
        match inode.info().format().layout() {
            Layout::FlatInline => self.flatmap(inode, offset, true),
            Layout::FlatPlain => self.flatmap(inode, offset, false),
            Layout::Chunk => self.chunk_map(inode, offset),
            _ => todo!(),
        }
    }

    // TODO:: Remove the Box<dyn Iterator> here
    // Maybe create another wrapper type and we implement the Iterator there?
    // Seems unachievable because of static dispatch of Buffer is not allowed at compile time
    // If we want to have trait object that can be exported to c_void
    // Leave it as it is for tradeoffs

    fn mapped_iter<'b, 'a: 'b>(
        &'a self,
        inode: &'b I,
        offset: Off,
    ) -> Box<dyn BufferMapIter<'a> + 'b>;

    fn continous_iter<'a>(&'a self, offset: Off, len: Off)
        -> Box<dyn ContinousBufferIter<'a> + 'a>;

    fn fill_dentries(&self, inode: &I, offset: Off, emitter: &mut dyn FnMut(Dirent<'_>, Off)) {
        if offset > inode.info().file_size() {
            return;
        }

        let map_offset = round!(DOWN, offset, self.blksz());
        let blk_offset = round!(UP, self.blkoff(offset), size_of::<DirentDesc>() as Off);

        let mut map_iter = self.mapped_iter(inode, map_offset);
        let first_buf = map_iter.next().unwrap();
        let mut collection = first_buf.iter_dir();

        let mut pos: Off = map_offset + blk_offset;

        if blk_offset as usize / size_of::<DirentDesc>() <= collection.total() {
            collection.skip_dir(blk_offset as usize / size_of::<DirentDesc>());
            for dirent in collection {
                emitter(dirent, pos);
                pos += size_of::<DirentDesc>() as Off;
            }
        }

        pos = round!(UP, pos, self.blksz());

        for buf in map_iter {
            for dirent in buf.iter_dir() {
                emitter(dirent, pos);
                pos += size_of::<DirentDesc>() as Off;
            }
            pos = round!(UP, pos, self.blksz());
        }
    }

    fn find_nid(&self, inode: &I, name: &str) -> Option<Nid> {
        for buf in self.mapped_iter(inode, 0) {
            for dirent in buf.iter_dir() {
                if dirent.dirname() == name.as_bytes() {
                    return Some(dirent.desc.nid);
                }
            }
        }
        None
    }

    fn get_xattr(&self, inode: &I, index: u32, name: &[u8], buffer: &mut [u8]) -> bool {
        let count = inode.info().xattr_count();
        let shared_count = inode.xattrs_header().shared_indexes.len();
        let inline_count = count as usize - shared_count;

        let inline_offset = self.iloc(inode.nid())
            + inode.info().inode_size() as Off
            + size_of::<DiskEntryIndexHeader>() as Off
            + shared_count as Off * 4;

        {
            let mut inline_provider =
                SkippableContinousIter::new(self.continous_iter(inline_offset, u64::MAX));
            for _ in 0..inline_count {
                let header = inline_provider.get_entry_header();
                if inline_provider.get_xattr_value(
                    self.xattr_prefixes(),
                    &header,
                    name,
                    index,
                    buffer,
                ) {
                    return true;
                }
            }
        }

        for index in inode.xattrs_header().shared_indexes.iter() {
            let mut provider = SkippableContinousIter::new(self.continous_iter(
                self.blkpos(self.superblock().xattr_blkaddr) + (*index as Off) * 4,
                u64::MAX,
            ));
            let header = provider.get_entry_header();
            if provider.get_xattr_value(self.xattr_prefixes(), &header, name, *index, buffer) {
                return true;
            }
        }
        false
    }

    fn list_xattrs(&self, inode: &I, buffer: &mut [u8]) {
        let count = inode.info().xattr_count();
        let shared_count = inode.xattrs_header().shared_indexes.len();
        let inline_count = count as usize - shared_count;
        let inline_offset = self.iloc(inode.nid())
            + inode.info().inode_size() as Off
            + size_of::<DiskEntryIndexHeader>() as Off
            + shared_count as Off * 4;

        let mut offset = 0;
        {
            let mut inline_provider =
                SkippableContinousIter::new(self.continous_iter(inline_offset, u64::MAX));
            for _ in 0..inline_count {
                let header = inline_provider.get_entry_header();
                offset += inline_provider.get_xattr_name(
                    self.xattr_prefixes(),
                    &header,
                    &mut buffer[offset..],
                );
            }
        }

        for index in inode.xattrs_header().shared_indexes.iter() {
            let mut provider = SkippableContinousIter::new(self.continous_iter(
                self.blkpos(self.superblock().xattr_blkaddr) + (*index as Off) * 4,
                u64::MAX,
            ));
            let header = provider.get_entry_header();
            offset +=
                provider.get_xattr_name(self.xattr_prefixes(), &header, &mut buffer[offset..]);
        }
    }
}

pub(crate) struct SuperblockInfo<I, C>
where
    I: Inode,
    C: InodeCollection<I = I>,
{
    pub(crate) filesystem: Box<dyn FileSystem<I>>,
    pub(crate) inodes: C,
}

impl<I, C> SuperblockInfo<I, C>
where
    I: Inode,
    C: InodeCollection<I = I>,
{
    pub(crate) fn new(fs: Box<dyn FileSystem<I>>, c: C) -> Self {
        Self {
            filesystem: fs,
            inodes: c,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    extern crate std;

    use alloc::string::ToString;

    use super::*;
    use crate::inode::tests::*;
    use crate::operations::*;
    use hex_literal::hex;
    use sha2::{Digest, Sha512};
    use std::collections::HashMap;
    use std::format;
    use std::fs::File;
    use std::path::Path;
    use std::vec;

    pub(crate) const SB_MAGIC: u32 = 0xE0F5E1E2;

    pub(crate) type SimpleBufferedFileSystem =
        SuperblockInfo<SimpleInode, HashMap<Nid, SimpleInode>>;

    pub(crate) fn load_fixtures() -> impl Iterator<Item = File> {
        vec![512, 1024, 2048, 4096].into_iter().map(|num| {
            let mut s = env!("CARGO_MANIFEST_DIR").to_string();
            s.push_str(&format!("/tests/sample_{num}.img"));
            File::options()
                .read(true)
                .write(true)
                .open(Path::new(&s))
                .unwrap()
        })
    }

    fn test_superblock_def(sbi: &mut SimpleBufferedFileSystem) {
        assert_eq!(sbi.filesystem.superblock().magic, SB_MAGIC);
    }

    fn test_filesystem_ilookup(sbi: &mut SimpleBufferedFileSystem) {
        const LIPSUM_HEX: [u8;64] = hex!("6846740fd4c03c86524d39e0012ec8eb1e4b87e8a90c65227904148bc0e4d0592c209151a736946133cd57f7ec59c4e8a445e7732322dda9ce356f8d0100c4ca");
        const LIPSUM_FILE_SIZE: u64 = 5060;
        const LIPSUM_TYPE: Type = Type::Regular;
        let inode = ilookup(&*sbi.filesystem, &mut sbi.inodes, "/texts/lipsum.txt").unwrap();
        assert_eq!(inode.info().inode_type(), LIPSUM_TYPE);
        assert_eq!(inode.info().file_size(), LIPSUM_FILE_SIZE);

        let mut hasher = Sha512::new();
        for block in sbi.filesystem.mapped_iter(inode, 0) {
            hasher.update(block.content());
        }
        let result = hasher.finalize();
        assert_eq!(result[..], LIPSUM_HEX);
    }

    fn test_continous_iter(sbi: &mut SimpleBufferedFileSystem) {
        const README_HEX: [u8; 64] = hex!("99fffc75aec028f417d9782fffed6c5d877a29ad1b16fc62bfeb168cdaf8db6db2bad1814904cd0fa18a2396c2c618041682a010601f4052b9895138d4ed6f16");
        const README_FILE_SIZE: u64 = 38;
        const README_TYPE: Type = Type::Regular;
        let inode = ilookup(&*sbi.filesystem, &mut sbi.inodes, "/README.md").unwrap();
        assert_eq!(inode.info().inode_type(), README_TYPE);
        assert_eq!(inode.info().file_size(), README_FILE_SIZE);
        let map = sbi.filesystem.map(inode, 0).unwrap();

        let mut hasher = Sha512::new();
        for block in sbi
            .filesystem
            .continous_iter(map.physical.start, map.physical.len)
        {
            hasher.update(block.content());
        }
        let result = hasher.finalize();
        assert_eq!(result[..], README_HEX);
    }

    pub(crate) fn test_filesystem(sbi: &mut SimpleBufferedFileSystem) {
        test_superblock_def(sbi);
        test_filesystem_ilookup(sbi);
        test_continous_iter(sbi);
    }

    #[test]
    fn test_superblock_size() {
        assert_eq!(core::mem::size_of::<SuperBlock>(), 128);
    }
}
