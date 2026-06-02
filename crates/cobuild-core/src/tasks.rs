use alloc::vec::Vec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureOrigin {
    SighashAll,
    OtxBase,
    OtxAppend,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockSignatureRequest {
    pub script_hash: [u8; 32],
    pub carrier_witness_index: usize,
    pub origin: SignatureOrigin,
    pub seal: Vec<u8>,
    pub signing_message_hash: [u8; 32],
}
