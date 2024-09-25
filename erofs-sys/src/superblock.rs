// Copyright 2024 Yiyang Wu SPDX-License-Identifier: MIT or GPL-2.0-or-later

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::round;

use super::alloc_helper::*;
use super::data::raw_iters::*;
use super::devices::*;
use super::dir::*;
use super::errnos::*;
use super::inode::*;
use super::map::*;
use super::xattrs::*;
use super::*;

use core::mem::size_of;

/// File based modules
pub mod file;
/// Mem based modules
pub mod mem;

/// The ondisk superblock structure.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct SuperBlock {
    pub(crate) magic: u32,
    pub(crate) checksum: i32,
    pub(crate) feature_compat: i32,
    pub(crate) blkszbits: u8,
    pub(crate) sb_extslots: u8,
    /// root_nid
    pub root_nid: i16,
    pub(crate) inos: i64,
    /// build_time
    pub build_time: i64,
    /// build_time_nsec
    pub build_time_nsec: i32,
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

impl TryFrom<&[u8]> for SuperBlock {
    type Error = core::array::TryFromSliceError;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        value[0..128].try_into()
    }
}

impl From<[u8; 128]> for SuperBlock {
    fn from(value: [u8; 128]) -> Self {
        Self {
            magic: u32::from_le_bytes([value[0], value[1], value[2], value[3]]),
            checksum: i32::from_le_bytes([value[4], value[5], value[6], value[7]]),
            feature_compat: i32::from_le_bytes([value[8], value[9], value[10], value[11]]),
            blkszbits: value[12],
            sb_extslots: value[13],
            root_nid: i16::from_le_bytes([value[14], value[15]]),
            inos: i64::from_le_bytes([
                value[16], value[17], value[18], value[19], value[20], value[21], value[22],
                value[23],
            ]),
            build_time: i64::from_le_bytes([
                value[24], value[25], value[26], value[27], value[28], value[29], value[30],
                value[31],
            ]),
            build_time_nsec: i32::from_le_bytes([value[32], value[33], value[34], value[35]]),
            blocks: i32::from_le_bytes([value[36], value[37], value[38], value[39]]),
            meta_blkaddr: u32::from_le_bytes([value[40], value[41], value[42], value[43]]),
            xattr_blkaddr: u32::from_le_bytes([value[44], value[45], value[46], value[47]]),
            uuid: value[48..64].try_into().unwrap(),
            volume_name: value[64..80].try_into().unwrap(),
            feature_incompat: i32::from_le_bytes([value[80], value[81], value[82], value[83]]),
            compression: i16::from_le_bytes([value[84], value[85]]),
            extra_devices: i16::from_le_bytes([value[86], value[87]]),
            devt_slotoff: i16::from_le_bytes([value[88], value[89]]),
            dirblkbits: value[90],
            xattr_prefix_count: value[91],
            xattr_prefix_start: i32::from_le_bytes([value[92], value[93], value[94], value[95]]),
            packed_nid: i64::from_le_bytes([
                value[96], value[97], value[98], value[99], value[100], value[101], value[102],
                value[103],
            ]),
            xattr_filter_reserved: value[104],
            reserved: value[105..128].try_into().unwrap(),
        }
    }
}

pub(crate) type SuperBlockBuf = [u8; size_of::<SuperBlock>()];
pub(crate) const SUPERBLOCK_EMPTY_BUF: SuperBlockBuf = [0; size_of::<SuperBlock>()];

/// Used for external address calculation.
pub(crate) struct Accessor {
    pub(crate) base: Off,
    pub(crate) off: Off,
    pub(crate) len: Off,
    pub(crate) nr: Off,
}

impl Accessor {
    pub(crate) fn new(address: Off, bits: Off) -> Self {
        let sz = 1 << bits;
        let mask = sz - 1;
        Accessor {
            base: (address >> bits) << bits,
            off: address & mask,
            len: sz - (address & mask),
            nr: address >> bits,
        }
    }
}

impl SuperBlock {
    pub(crate) fn blk_access(&self, address: Off) -> Accessor {
        Accessor::new(address, self.blkszbits as Off)
    }

    pub(crate) fn blknr(&self, pos: Off) -> Blk {
        (pos >> self.blkszbits) as Blk
    }

    pub(crate) fn blkpos(&self, blk: Blk) -> Off {
        (blk as Off) << self.blkszbits
    }

    /// blksz
    pub fn blksz(&self) -> Off {
        1 << self.blkszbits
    }

    /// blk_round_up
    pub fn blk_round_up(&self, addr: Off) -> Blk {
        ((addr + self.blksz() - 1) >> self.blkszbits) as Blk
    }

    /// generic round up
    pub fn blk_round_up_generic(&self, size: Off) -> Blk {
        ((size + self.blksz() - 1) >> 9) as Blk
    }

    pub(crate) fn iloc(&self, nid: Nid) -> Off {
        self.blkpos(self.meta_blkaddr) + ((nid as Off) << (5 as Off))
    }

    pub(crate) fn chunk_access(&self, format: ChunkFormat, address: Off) -> Accessor {
        let chunkbits = format.chunkbits() + self.blkszbits as u16;
        Accessor::new(address, chunkbits as Off)
    }
}

/// FileSystem trait
pub trait FileSystem<I>
where
    I: Inode,
{
    /// Superblock
    fn superblock(&self) -> &SuperBlock;
    /// Backend
    fn backend(&self) -> &dyn Backend;
    /// As_filesystem
    fn as_filesystem(&self) -> &dyn FileSystem<I>;

    // block map goes here.
    /// DeviceInfo
    fn device_info(&self) -> &DeviceInfo;
    /// Flatmap
    fn flatmap(&self, inode: &I, offset: Off, inline: bool) -> MapResult {
        let sb = self.superblock();
        let nblocks = sb.blk_round_up(inode.info().file_size());
        let blkaddr = match inode.info().spec() {
            Spec::RawBlk(blkaddr) => Ok(blkaddr),
            _ => Err(EUCLEAN),
        }?;

        let lastblk = if inline { nblocks - 1 } else { nblocks };
        if offset < sb.blkpos(lastblk) {
            let len = inode.info().file_size().min(sb.blkpos(lastblk)) - offset;
            Ok(Map {
                logical: Segment { start: offset, len },
                physical: Segment {
                    start: sb.blkpos(blkaddr) + offset,
                    len,
                },
                algorithm_format: 0,
                device_id: 0,
                map_type: MapType::Normal,
            })
        } else if inline {
            let len = inode.info().file_size() - offset;
            let accessor = sb.blk_access(offset);
            Ok(Map {
                logical: Segment { start: offset, len },
                physical: Segment {
                    start: sb.iloc(inode.nid())
                        + inode.info().inode_size()
                        + inode.info().xattr_size()
                        + accessor.off,
                    len,
                },
                algorithm_format: 0,
                device_id: 0,
                map_type: MapType::Meta,
            })
        } else {
            Err(EUCLEAN)
        }
    }
    /// ChunkMap
    fn chunk_map(&self, inode: &I, offset: Off) -> MapResult {
        let sb = self.superblock();
        let chunkformat = match inode.info().spec() {
            Spec::Chunk(chunkformat) => Ok(chunkformat),
            _ => Err(EUCLEAN),
        }?;
        let accessor = sb.chunk_access(chunkformat, offset);

        if chunkformat.is_chunkindex() {
            let unit = size_of::<ChunkIndex>() as Off;
            let pos = round!(
                UP,
                self.superblock().iloc(inode.nid())
                    + inode.info().inode_size()
                    + inode.info().xattr_size()
                    + unit * accessor.nr,
                unit
            );
            let mut buf = [0u8; size_of::<ChunkIndex>()];
            self.backend().fill(&mut buf, 0, pos)?;
            let chunk_index = ChunkIndex::from(buf);
            if chunk_index.blkaddr == u32::MAX {
                Err(EUCLEAN)
            } else {
                Ok(Map {
                    logical: Segment {
                        start: accessor.base + accessor.off,
                        len: accessor.len,
                    },
                    physical: Segment {
                        start: sb.blkpos(chunk_index.blkaddr) + accessor.off,
                        len: accessor.len,
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
                sb.iloc(inode.nid())
                    + inode.info().inode_size()
                    + inode.info().xattr_size()
                    + unit * accessor.nr,
                unit
            );
            let mut buf = [0u8; 4];
            self.backend().fill(&mut buf, 0, pos)?;
            let blkaddr = u32::from_le_bytes(buf);
            let len = accessor.len.min(inode.info().file_size() - offset);
            if blkaddr == u32::MAX {
                Err(EUCLEAN)
            } else {
                Ok(Map {
                    logical: Segment {
                        start: accessor.base + accessor.off,
                        len,
                    },
                    physical: Segment {
                        start: sb.blkpos(blkaddr) + accessor.off,
                        len,
                    },
                    algorithm_format: 0,
                    device_id: 0,
                    map_type: MapType::Normal,
                })
            }
        }
    }

    /// Map
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

    /// Map_Iter
    fn mapped_iter<'b, 'a: 'b>(
        &'a self,
        inode: &'b I,
        offset: Off,
    ) -> PosixResult<Box<dyn BufferMapIter<'a> + 'b>>;

    /// ContinousIter
    fn continuous_iter<'a>(
        &'a self,
        offset: Off,
        len: Off,
    ) -> PosixResult<Box<dyn ContinuousBufferIter<'a> + 'a>>;

    /// ReadInodeInfo
    fn read_inode_info(&self, nid: Nid) -> PosixResult<InodeInfo> {
        (self.as_filesystem(), nid).try_into()
    }
    /// Findnid
    fn find_nid(&self, inode: &I, name: &str) -> PosixResult<Option<Nid>> {
        for buf in self.mapped_iter(inode, 0)? {
            for dirent in buf?.iter_dir() {
                if dirent.dirname() == name.as_bytes() {
                    return Ok(Some(dirent.desc.nid));
                }
            }
        }
        Ok(None)
    }

    // Readdir related goes here.
    /// FillDentries
    fn fill_dentries(
        &self,
        inode: &I,
        offset: Off,
        skipents: u64,
        emitter: &mut dyn FnMut(Dirent<'_>, Off) -> bool,
    ) -> PosixResult<()> {
        let sb = self.superblock();
        let accessor = sb.blk_access(offset);
        if offset > inode.info().file_size() {
            return Err(EUCLEAN);
        }

        let map_offset = round!(DOWN, offset, sb.blksz());
        let blk_offset = round!(UP, accessor.off, size_of::<DirentDesc>() as Off);

        let mut map_iter = self.mapped_iter(inode, map_offset)?;
        let first_buf = map_iter.next().unwrap()?;
        let mut collection = first_buf.iter_dir();
        let mut cnt = 0;
        let mut pos: Off = map_offset + blk_offset;

        if blk_offset as usize / size_of::<DirentDesc>() <= collection.total() {
            collection.skip_dir(blk_offset as usize / size_of::<DirentDesc>());
            for dirent in collection {
                if cnt >= skipents && emitter(dirent, pos) {
                    return Ok(());
                }
                pos += size_of::<DirentDesc>() as Off;
                cnt += 1;
            }
        }

        pos = round!(UP, pos, sb.blksz());

        for buf in map_iter {
            for dirent in buf?.iter_dir() {
                if cnt >= skipents && emitter(dirent, pos) {
                    return Ok(());
                }
                pos += size_of::<DirentDesc>() as Off;
                cnt += 1;
            }
            pos = round!(UP, pos, sb.blksz());
        }
        Ok(())
    }

    // Extended attributes goes here.
    /// XattrInfixes
    fn xattr_infixes(&self) -> &Vec<XAttrInfix>;
    // Currently we eagerly initialized all xattrs;
    /// xattrs
    fn read_inode_xattrs_shared_entries(
        &self,
        nid: Nid,
        info: &InodeInfo,
    ) -> PosixResult<XAttrSharedEntries> {
        let sb = self.superblock();
        let mut offset = sb.iloc(nid) + info.inode_size();
        let mut buf = XATTR_ENTRY_SUMMARY_BUF;
        let mut indexes: Vec<u32> = Vec::new();
        self.backend().fill(&mut buf, 0, offset)?;

        let header: XAttrSharedEntrySummary = XAttrSharedEntrySummary::from(buf);
        offset += size_of::<XAttrSharedEntrySummary>() as Off;
        for buf in self.continuous_iter(offset, (header.shared_count << 2) as Off)? {
            let data = buf?;
            extend_from_slice(&mut indexes, unsafe {
                core::slice::from_raw_parts(
                    data.content().as_ptr().cast(),
                    data.content().len() >> 2,
                )
            })?;
        }

        Ok(XAttrSharedEntries {
            name_filter: header.name_filter,
            shared_indexes: indexes,
        })
    }
    /// get_xattr
    fn get_xattr(
        &self,
        inode: &I,
        index: u32,
        name: &[u8],
        buffer: &mut Option<&mut [u8]>,
    ) -> PosixResult<XAttrValue> {
        let sb = self.superblock();
        let shared_count = inode.xattrs_shared_entries().shared_indexes.len();
        let inline_offset = sb.iloc(inode.nid())
            + inode.info().inode_size() as Off
            + size_of::<XAttrSharedEntrySummary>() as Off
            + 4 * shared_count as Off;

        let inline_len = inode.info().xattr_size()
            - size_of::<XAttrSharedEntrySummary>() as Off
            - shared_count as Off * 4;

        if let Some(mut inline_provider) =
            SkippableContinuousIter::try_new(self.continuous_iter(inline_offset, inline_len)?)?
        {
            while !inline_provider.eof() {
                let header = inline_provider.get_entry_header()?;
                match inline_provider.query_xattr_value(
                    self.xattr_infixes(),
                    &header,
                    name,
                    index,
                    buffer,
                ) {
                    Ok(value) => return Ok(value),
                    Err(e) => {
                        if e != ENODATA {
                            return Err(e);
                        }
                    }
                }
            }
        }

        for entry_index in inode.xattrs_shared_entries().shared_indexes.iter() {
            let mut shared_provider = SkippableContinuousIter::try_new(self.continuous_iter(
                sb.blkpos(self.superblock().xattr_blkaddr) + (*entry_index as Off) * 4,
                u64::MAX,
            )?)?
            .unwrap();
            let header = shared_provider.get_entry_header()?;
            match shared_provider.query_xattr_value(
                self.xattr_infixes(),
                &header,
                name,
                index,
                buffer,
            ) {
                Ok(value) => return Ok(value),
                Err(e) => {
                    if e != ENODATA {
                        return Err(e);
                    }
                }
            }
        }

        Err(ENODATA)
    }
    /// list_xattrs
    fn list_xattrs(&self, inode: &I, buffer: &mut [u8]) -> PosixResult<usize> {
        let sb = self.superblock();
        let shared_count = inode.xattrs_shared_entries().shared_indexes.len();
        let inline_offset = sb.iloc(inode.nid())
            + inode.info().inode_size() as Off
            + size_of::<XAttrSharedEntrySummary>() as Off
            + shared_count as Off * 4;
        let mut offset = 0;
        let inline_len = inode.info().xattr_size()
            - size_of::<XAttrSharedEntrySummary>() as Off
            - shared_count as Off * 4;

        if let Some(mut inline_provider) =
            SkippableContinuousIter::try_new(self.continuous_iter(inline_offset, inline_len)?)?
        {
            while !inline_provider.eof() {
                let header = inline_provider.get_entry_header()?;
                offset += inline_provider.get_xattr_key(
                    self.xattr_infixes(),
                    &header,
                    &mut buffer[offset..],
                )?;
                inline_provider.skip_xattr_value(&header)?;
            }
        }

        for index in inode.xattrs_shared_entries().shared_indexes.iter() {
            let mut shared_provider = SkippableContinuousIter::try_new(self.continuous_iter(
                sb.blkpos(self.superblock().xattr_blkaddr) + (*index as Off) * 4,
                u64::MAX,
            )?)?
            .unwrap();
            let header = shared_provider.get_entry_header()?;
            offset += shared_provider.get_xattr_key(
                self.xattr_infixes(),
                &header,
                &mut buffer[offset..],
            )?;
        }
        Ok(offset)
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

    fn test_filesystem_ilookup1(sbi: &mut SimpleBufferedFileSystem) {
        const LIPSUM_HEX: [u8;64] = hex!("6846740fd4c03c86524d39e0012ec8eb1e4b87e8a90c65227904148bc0e4d0592c209151a736946133cd57f7ec59c4e8a445e7732322dda9ce356f8d0100c4ca");
        const LIPSUM_FILE_SIZE: u64 = 5060;
        const LIPSUM_TYPE: Type = Type::Regular;
        let inode = lookup(
            &*sbi.filesystem,
            &mut sbi.inodes,
            sbi.filesystem.superblock().root_nid as Nid,
            "/texts/lipsum.txt",
        )
        .unwrap();
        assert_eq!(inode.info().inode_type(), LIPSUM_TYPE);
        assert_eq!(inode.info().file_size(), LIPSUM_FILE_SIZE);

        let mut hasher = Sha512::new();
        for block in sbi.filesystem.mapped_iter(inode, 0).unwrap() {
            hasher.update(block.unwrap().content());
        }
        let result = hasher.finalize();
        assert_eq!(result[..], LIPSUM_HEX);
    }

    fn test_filesystem_ilookup2(sbi: &mut SimpleBufferedFileSystem) {
        const IMAGE_HEX: [u8;64] = hex!("2d0f63b3ca997d30d65f70f32bf97038d92d1f4e642fe48ede697ab73e936e5b2bff9b556a786340d9385993b2e7f6744cbbf8b4660c55b33c907a3a2ced33b5");
        const IMAGE_FILE_SIZE: u64 = 13735;
        const IMAGE_TYPE: Type = Type::Regular;

        let inode = lookup(
            &*sbi.filesystem,
            &mut sbi.inodes,
            sbi.filesystem.superblock().root_nid as Nid,
            "/images/inabukumori.jpg",
        )
        .unwrap();
        assert_eq!(inode.info().inode_type(), IMAGE_TYPE);
        assert_eq!(inode.info().file_size(), IMAGE_FILE_SIZE);
        let mut hasher = Sha512::new();
        for block in sbi.filesystem.mapped_iter(inode, 0).unwrap() {
            hasher.update(block.unwrap().content());
        }
        let result = hasher.finalize();
        assert_eq!(result[..], IMAGE_HEX);
    }
    fn test_continous_iter(sbi: &mut SimpleBufferedFileSystem) {
        const README_CHECKSUM: [u8; 64] = hex!("99fffc75aec028f417d9782fffed6c5d877a29ad1b16fc62bfeb168cdaf8db6db2bad1814904cd0fa18a2396c2c618041682a010601f4052b9895138d4ed6f16");
        const README_FILE_SIZE: u64 = 38;
        const README_TYPE: Type = Type::Regular;
        let inode = lookup(
            &*sbi.filesystem,
            &mut sbi.inodes,
            sbi.filesystem.superblock().root_nid as Nid,
            "/README.md",
        )
        .unwrap();
        assert_eq!(inode.info().inode_type(), README_TYPE);
        assert_eq!(inode.info().file_size(), README_FILE_SIZE);
        let map = sbi.filesystem.map(inode, 0).unwrap();

        let mut hasher = Sha512::new();
        for block in sbi
            .filesystem
            .continuous_iter(map.physical.start, map.physical.len)
            .unwrap()
        {
            hasher.update(block.unwrap().content());
        }
        let result = hasher.finalize();
        assert_eq!(result[..], README_CHECKSUM);
    }

    fn test_get_file_xattr(sbi: &mut SimpleBufferedFileSystem) {
        const README_SHA512_LITERAL: &[u8] = b"99fffc75aec028f417d9782fffed6c5d877a29ad1b16fc62bfeb168cdaf8db6db2bad1814904cd0fa18a2396c2c618041682a010601f4052b9895138d4ed6f16";
        const README_SHA512HMAC_LITERAL: &[u8] = b"45d111b7dc1799cc9c4f9989b301cac37c7ba66f5cfb559566c407f7f9476e2596b53d345045d426d9144eaabb9f55abb05f03b1ff44d69081831b19c87cb2d3";

        let inode = lookup(
            &*sbi.filesystem,
            &mut sbi.inodes,
            sbi.filesystem.superblock().root_nid as Nid,
            "/README.md",
        )
        .unwrap();

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
            .is_err_and(|x| x == Errno::ENODATA));
    }

    fn test_get_dir_xattr(sbi: &mut SimpleBufferedFileSystem) {
        let inode = lookup(
            &*sbi.filesystem,
            &mut sbi.inodes,
            sbi.filesystem.superblock().root_nid as Nid,
            "/",
        )
        .unwrap();
        assert!(sbi
            .filesystem
            .get_xattr(inode, 2, b"", &mut None)
            .is_err_and(|x| x == Errno::ENODATA));
    }

    fn test_list_xattr(sbi: &mut SimpleBufferedFileSystem) {
        let mut result = [0u8; 512];
        let inode = lookup(
            &*sbi.filesystem,
            &mut sbi.inodes,
            sbi.filesystem.superblock().root_nid as Nid,
            "/README.md",
        )
        .unwrap();
        let length = sbi.filesystem.list_xattrs(inode, &mut result).unwrap();
        assert_eq!(
            &result[..length],
            b"user.sha512sum\0user.sha512hmac\0security.selinux\0"
        );
    }

    pub(crate) fn test_filesystem(sbi: &mut SimpleBufferedFileSystem) {
        test_superblock_def(sbi);
        test_filesystem_ilookup1(sbi);
        test_filesystem_ilookup2(sbi);
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
