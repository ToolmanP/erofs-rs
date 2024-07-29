// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-only

use super::*;

pub(crate) const MAP_MAPPED: u32 = 0x0001;
pub(crate) const MAP_META: u32 = 0x0002;
pub(crate) const MAP_ENCODED: u32 = 0x0004;
pub(crate) const MAP_FULL_MAPPED: u32 = 0x0008;
pub(crate) const MAP_FRAGMENT: u32 = 0x0010;
pub(crate) const MAP_PARTIAL_REF: u32 = 0x0020;

#[derive(Debug, Default)]
#[repr(C)]
pub(crate) struct AddressMap {
    pub(crate) start: Off,
    pub(crate) len: Off,
}

#[derive(Debug, Default)]
#[repr(C)]
pub(crate) struct Map {
    pub(crate) logical: AddressMap,
    pub(crate) physical: AddressMap,
    pub(crate) device_id: u16,
    pub(crate) algorithm_format: u16,
    pub(crate) flags: u32,
}
