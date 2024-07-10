use crate::*;

#[derive(Debug)]
pub(crate) struct AddressMap {
    pub(crate) start: Off,
    pub(crate) len: Off,
}

#[derive(Debug)]
pub(crate) struct Map {
    pub(crate) index: Blk,
    pub(crate) offset: Off,
    pub(crate) logical: AddressMap,
    pub(crate) physical: AddressMap,
}
