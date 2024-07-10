#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub enum SuperblockCompressionInfo {
    AvailableComprAlgs(u16),
    Lz4MaxDistance(u16),
}


#[allow(dead_code)]
pub enum InodeCompressionInfo {

}

impl Default for SuperblockCompressionInfo{
    fn default() -> Self {
        Self::AvailableComprAlgs(0)
    }
}
