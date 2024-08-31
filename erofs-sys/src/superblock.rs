// Copyright 2024 Yiyang Wu SPDX-License-Identifier: MIT or GPL-2.0-later

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::round;

use super::alloc_helper::*;
use super::data::*;
use super::devices::*;
use super::dir::*;
use super::inode::*;
use super::map::*;
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
pub(crate) struct DiskBlockAccessor {
    pub(crate) base: Off,
    pub(crate) off: Off,
    pub(crate) len: Off,
}

impl DiskBlockAccessor {
    pub(crate) fn new(sb: &SuperBlock, address: Off) -> Self {
        let bits = sb.blkszbits as Off;
        let sz = 1 << bits;
        let mask = sz - 1;
        DiskBlockAccessor {
            base: (address >> bits) << bits,
            off: address & mask,
            len: sz - (address & mask),
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

    fn xattr_infixes(&self) -> &Vec<xattrs::XAttrInfix>;

    // Currently we eagerly initialized all xattrs;
    //
    fn read_inode_xattrs_shared_entries(&self, nid: Nid, info: &InodeInfo) -> XAttrSharedEntries {
        let mut offset = self.iloc(nid) + info.inode_size();

        let mut buf = XATTR_ENTRY_SUMMARY_BUF;
        let mut indexes: Vec<u32> = Vec::new();
        self.backend().fill(&mut buf, offset).unwrap();
        let header: XAttrSharedEntrySummary = XAttrSharedEntrySummary::from(buf);
        offset += size_of::<XAttrSharedEntrySummary>() as Off;

        for buf in self.continous_iter(offset, (header.shared_count << 2) as Off) {
            let data = buf.content();
            extend_from_slice(&mut indexes, unsafe {
                core::slice::from_raw_parts(data.as_ptr().cast(), data.len() >> 2)
            });
        }

        XAttrSharedEntries {
            name_filter: header.name_filter,
            shared_indexes: indexes,
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
        if offset < self.blkpos(lastblk) {
            let len = inode.info().file_size().min(self.blkpos(lastblk)) - offset;
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
            let len = inode.info().file_size() - offset;
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
        let chunkformat = match inode.info().spec() {
            Spec::Data(dataspec) => match dataspec {
                DataSpec::Chunk(chunkformat) => chunkformat,
                _ => unimplemented!(),
            },
            _ => unimplemented!(),
        };

        let chunkbits = (chunkformat.chunkbits() + self.superblock().blkszbits as u16) as Off;
        let chunknr = offset >> chunkbits;
        let chunkoff = offset & ((1 << chunkbits) - 1);

        if chunkformat.is_chunkindex() {
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
                        start: (chunknr << chunkbits) + chunkoff,
                        len: 1 << chunkbits,
                    },
                    physical: Segment {
                        start: self.blkpos(chunk_index.blkaddr) + chunkoff,
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
            let len = (1 << chunkbits).min(inode.info().file_size() - offset);
            if blkaddr == u32::MAX {
                Err(MapError::OutofBound)
            } else {
                Ok(Map {
                    logical: Segment {
                        start: (chunknr << chunkbits) + chunkoff,
                        len,
                    },
                    physical: Segment {
                        start: self.blkpos(blkaddr) + chunkoff,
                        len,
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

    fn get_xattr(
        &self,
        inode: &I,
        index: u32,
        name: &[u8],
        buffer: &mut Option<&mut [u8]>,
    ) -> XAttrResult {
        let shared_count = inode.xattrs_shared_entries().shared_indexes.len();

        let inline_offset = self.iloc(inode.nid())
            + inode.info().inode_size() as Off
            + size_of::<XAttrSharedEntrySummary>() as Off
            + 4 * shared_count as Off;

        let inline_len = inode.info().xattr_size()
            - size_of::<XAttrSharedEntrySummary>() as Off
            - shared_count as Off * 4;

        if let Some(mut inline_provider) =
            SkippableContinousIter::try_new(self.continous_iter(inline_offset, inline_len))
        {
            while !inline_provider.eof() {
                let header = inline_provider.get_entry_header();
                let result = inline_provider.query_xattr_value(
                    self.xattr_infixes(),
                    &header,
                    name,
                    index,
                    buffer,
                );
                if result.is_ok() {
                    return result;
                }
            }
        }

        for entry_index in inode.xattrs_shared_entries().shared_indexes.iter() {
            let mut shared_provider = SkippableContinousIter::try_new(self.continous_iter(
                self.blkpos(self.superblock().xattr_blkaddr) + (*entry_index as Off) * 4,
                u64::MAX,
            ))
            .unwrap();
            let header = shared_provider.get_entry_header();
            let result = shared_provider.query_xattr_value(
                self.xattr_infixes(),
                &header,
                name,
                index,
                buffer,
            );
            if result.is_ok() {
                return result;
            }
        }

        Err(XAttrError::NotFound)
    }

    fn list_xattrs(&self, inode: &I, buffer: &mut [u8]) -> usize {
        let shared_count = inode.xattrs_shared_entries().shared_indexes.len();
        let inline_offset = self.iloc(inode.nid())
            + inode.info().inode_size() as Off
            + size_of::<XAttrSharedEntrySummary>() as Off
            + shared_count as Off * 4;
        let mut offset = 0;
        let inline_len = inode.info().xattr_size()
            - size_of::<XAttrSharedEntrySummary>() as Off
            - shared_count as Off * 4;

        if let Some(mut inline_provider) =
            SkippableContinousIter::try_new(self.continous_iter(inline_offset, inline_len))
        {
            while !inline_provider.eof() {
                let header = inline_provider.get_entry_header();
                offset += inline_provider.get_xattr_key(
                    self.xattr_infixes(),
                    &header,
                    &mut buffer[offset..],
                );
                inline_provider.skip_xattr_value(&header);
            }
        }

        for index in inode.xattrs_shared_entries().shared_indexes.iter() {
            let mut shared_provider = SkippableContinousIter::try_new(self.continous_iter(
                self.blkpos(self.superblock().xattr_blkaddr) + (*index as Off) * 4,
                u64::MAX,
            ))
            .unwrap();
            let header = shared_provider.get_entry_header();
            offset +=
                shared_provider.get_xattr_key(self.xattr_infixes(), &header, &mut buffer[offset..]);
        }
        offset
    }
}

pub(crate) struct SuperblockInfo<I, C, T>
where
    I: Inode,
    C: InodeCollection<I = I>,
{
    pub(crate) filesystem: Box<dyn FileSystem<I>>,
    pub(crate) inodes: C,
    pub(crate) opaque: T,
}

impl<I, C, T> SuperblockInfo<I, C, T>
where
    I: Inode,
    C: InodeCollection<I = I>,
{
    pub(crate) fn new(fs: Box<dyn FileSystem<I>>, c: C, opaque: T) -> Self {
        Self {
            filesystem: fs,
            inodes: c,
            opaque,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    extern crate std;

    use super::inode::tests::*;
    use super::operations::*;
    use super::*;

    use hex_literal::hex;
    use sha2::{Digest, Sha512};
    use std::collections::HashMap;
    use std::format;
    use std::fs::File;
    use std::path::Path;
    use std::string::ToString;
    use std::vec;

    pub(crate) const SB_MAGIC: u32 = 0xE0F5E1E2;

    pub(crate) type SimpleBufferedFileSystem =
        SuperblockInfo<SimpleInode, HashMap<Nid, SimpleInode>, ()>;

    pub(crate) fn load_fixtures() -> impl Iterator<Item = File> {
        let flat = vec![512, 1024, 2048, 4096].into_iter().map(|num| {
            let mut s = env!("CARGO_MANIFEST_DIR").to_string();
            s.push_str(&format!("/tests/sample_{num}.img"));
            File::options()
                .read(true)
                .write(true)
                .open(Path::new(&s))
                .unwrap()
        });

        let chunk = vec![1024].into_iter().map(|num| {
            let mut s = env!("CARGO_MANIFEST_DIR").to_string();
            s.push_str(&format!("/tests/sample_512_{num}.img"));
            File::options()
                .read(true)
                .write(true)
                .open(Path::new(&s))
                .unwrap()
        });
        flat.chain(chunk)
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
        const README_CHECKSUM: [u8; 64] = hex!("99fffc75aec028f417d9782fffed6c5d877a29ad1b16fc62bfeb168cdaf8db6db2bad1814904cd0fa18a2396c2c618041682a010601f4052b9895138d4ed6f16");
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
        assert_eq!(result[..], README_CHECKSUM);
    }

    fn test_get_file_xattr(sbi: &mut SimpleBufferedFileSystem) {
        const README_SHA512_LITERAL: &[u8] = b"99fffc75aec028f417d9782fffed6c5d877a29ad1b16fc62bfeb168cdaf8db6db2bad1814904cd0fa18a2396c2c618041682a010601f4052b9895138d4ed6f16";
        const README_SHA512HMAC_LITERAL: &[u8] = b"45d111b7dc1799cc9c4f9989b301cac37c7ba66f5cfb559566c407f7f9476e2596b53d345045d426d9144eaabb9f55abb05f03b1ff44d69081831b19c87cb2d3";

        let inode = ilookup(&*sbi.filesystem, &mut sbi.inodes, "/README.md").unwrap();

        {
            let mut sha512 = [0u8; 128];
            if let XAttrValue::Vec(_b) = sbi
                .filesystem
                .get_xattr(inode, 1, b"sha512sum", &mut Some(&mut sha512))
                .unwrap()
            {
                panic!();
            }

            assert_eq!(sha512, README_SHA512_LITERAL);

            if let XAttrValue::Vec(b) = sbi
                .filesystem
                .get_xattr(inode, 1, b"sha512sum", &mut None)
                .unwrap()
            {
                assert_eq!(b, README_SHA512_LITERAL);
            } else {
                panic!();
            }
        }

        {
            assert!(sbi
                .filesystem
                .get_xattr(inode, 6, b"selinux", &mut Some(&mut [0u8; 128]))
                .is_ok())
        }

        assert!(sbi
            .filesystem
            .get_xattr(inode, 2, b"", &mut None)
            .is_err_and(|x| x == XAttrError::NotFound));
    }

    fn test_get_dir_xattr(sbi: &mut SimpleBufferedFileSystem) {
        let inode = ilookup(&*sbi.filesystem, &mut sbi.inodes, "/").unwrap();
        assert!(sbi
            .filesystem
            .get_xattr(inode, 2, b"", &mut None)
            .is_err_and(|x| x == XAttrError::NotFound));
    }

    fn test_list_xattr(sbi: &mut SimpleBufferedFileSystem) {
        let mut result = [0u8; 512];
        let inode = ilookup(&*sbi.filesystem, &mut sbi.inodes, "/README.md").unwrap();
        let length = sbi.filesystem.list_xattrs(inode, &mut result);
        assert_eq!(
            &result[..length],
            b"user.sha512sum\0user.sha512hmac\0security.selinux\0"
        );
    }

    pub(crate) fn test_filesystem(sbi: &mut SimpleBufferedFileSystem) {
        test_superblock_def(sbi);
        test_filesystem_ilookup(sbi);
        test_continous_iter(sbi);
        test_get_file_xattr(sbi);
        test_get_dir_xattr(sbi);
        test_list_xattr(sbi);
    }

    #[test]
    fn test_superblock_size() {
        assert_eq!(core::mem::size_of::<SuperBlock>(), 128);
    }
}
