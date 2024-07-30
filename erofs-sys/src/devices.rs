// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use super::alloc_helper::*;
use super::data::*;
use alloc::vec::Vec;

#[derive(Copy, Clone, Debug)]
pub(crate) struct DeviceSpec {
    pub(crate) tags: [u8; 64],
    pub(crate) blocks: u32,
    pub(crate) mapped_blocks: u32,
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub(crate) struct DeviceSlot {
    tags: [u8; 64],
    blocks: u32,
    mapped_blocks: u32,
    reserved: [u8; 56],
}

pub(crate) struct DeviceInfo {
    pub(crate) mask: u16,
    pub(crate) specs: Vec<DeviceSpec>,
}

pub(crate) fn get_device_infos<'a>(iter: &mut (dyn ContinousBufferIter<'a> + 'a)) -> DeviceInfo {
    let mut specs = Vec::new();
    for data in iter {
        let slots = unsafe {
            core::slice::from_raw_parts(
                data.content().as_ptr() as *const DeviceSlot,
                data.content().len() >> 7,
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
            );
        }
    }
    DeviceInfo {
        mask: specs.len().next_power_of_two() as u16,
        specs,
    }
}
