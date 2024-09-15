// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-or-later

use super::alloc_helper::*;
use super::data::raw_iters::*;
use super::*;
use alloc::vec::Vec;

/// Device specification.
#[derive(Copy, Clone, Debug)]
pub(crate) struct DeviceSpec {
    pub(crate) tags: [u8; 64],
    pub(crate) blocks: u32,
    pub(crate) mapped_blocks: u32,
}

/// Device slot.
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub(crate) struct DeviceSlot {
    tags: [u8; 64],
    blocks: u32,
    mapped_blocks: u32,
    reserved: [u8; 56],
}

impl From<[u8; 128]> for DeviceSlot {
    fn from(data: [u8; 128]) -> Self {
        Self {
            tags: data[0..64].try_into().unwrap(),
            blocks: u32::from_le_bytes(data[64..68].try_into().unwrap()),
            mapped_blocks: u32::from_le_bytes(data[68..72].try_into().unwrap()),
            reserved: data[72..128].try_into().unwrap(),
        }
    }
}

/// Device information.
pub(crate) struct DeviceInfo {
    pub(crate) mask: u16,
    pub(crate) specs: Vec<DeviceSpec>,
}

pub(crate) fn get_device_infos<'a>(
    iter: &mut (dyn ContinuousBufferIter<'a> + 'a),
) -> PosixResult<DeviceInfo> {
    let mut specs = Vec::new();
    for data in iter {
        let buffer = data?;
        let mut cur: usize = 0;
        let len = buffer.content().len();
        while cur + 128 <= len {
            let slot_data: [u8; 128] = buffer.content()[cur..cur + 128].try_into().unwrap();
            let slot = DeviceSlot::from(slot_data);
            cur += 128;
            push_vec(
                &mut specs,
                DeviceSpec {
                    tags: slot.tags,
                    blocks: slot.blocks,
                    mapped_blocks: slot.mapped_blocks,
                },
            )?;
        }
    }

    let mask = if specs.is_empty() {
        0
    } else {
        (1 << (specs.len().ilog2() + 1)) - 1
    };

    Ok(DeviceInfo { mask, specs })
}
