use alloc::vec::Vec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TxLevelLockTask {
    pub script_hash: [u8; 32],
    pub carrier_witness_index: usize,
    pub seal: Vec<u8>,
    pub signing_message_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OtxScope {
    Base,
    Append,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OtxLockTask {
    pub script_hash: [u8; 32],
    pub carrier_witness_index: usize,
    pub scope: OtxScope,
    pub seal: Vec<u8>,
    pub signing_message_hash: [u8; 32],
}
