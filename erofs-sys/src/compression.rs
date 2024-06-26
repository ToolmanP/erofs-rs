#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub enum CompressionInfo {
    AvailableComprAlgs(u16),
    Lz4MaxDistance(u16),
}

impl Default for CompressionInfo{
    fn default() -> Self {
        Self::AvailableComprAlgs(0)
    }
}
