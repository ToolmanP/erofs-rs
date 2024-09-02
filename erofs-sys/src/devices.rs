// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

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

/// Device information.
pub(crate) struct DeviceInfo {
    pub(crate) mask: u16,
    pub(crate) specs: Vec<DeviceSpec>,
}

pub(crate) fn get_device_infos<'a>(
    iter: &mut (dyn ContinousBufferIter<'a> + 'a),
) -> PosixResult<DeviceInfo> {
    let mut specs = Vec::new();
    for data in iter {
        let buffer = data?;
        let slots = unsafe {
            core::slice::from_raw_parts(
                buffer.content().as_ptr() as *const DeviceSlot,
                buffer.content().len() >> 7,
            )
        };
        for slot in slots {
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
