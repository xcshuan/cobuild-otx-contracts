use alloc::vec::Vec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TxLevelLockTask {
    pub script_hash: [u8; 32],
    pub carrier_witness_index: usize,
    pub seal: Vec<u8>,
    pub signing_message_hash: [u8; 32],
}
