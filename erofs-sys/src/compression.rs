// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-later

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) enum SuperblockCompressionInfo {
    AvailableComprAlgs(u16),
    Lz4MaxDistance(u16),
}

#[allow(dead_code)]
pub(crate) enum InodeCompressionInfo {}

impl Default for SuperblockCompressionInfo {
    fn default() -> Self {
        Self::AvailableComprAlgs(0)
    }
}
