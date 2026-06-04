use alloc::vec::Vec;

use crate::{layout::Range, view::MessageView};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockValidationPlan {
    pub lock_script_hash: [u8; 32],
    pub required_signatures: Vec<SigningRequirement>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SigningRequirement {
    pub origin: SignatureOrigin,
    pub carrier_witness_index: usize,
    pub seal: Vec<u8>,
    pub signing_message_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignatureOrigin {
    TxLevel,
    OtxBase,
    OtxAppend,
}

#[derive(Clone)]
pub struct TypeValidationPlan {
    pub type_script_hash: [u8; 32],
    pub related_messages: Vec<RelatedMessage>,
}

#[derive(Clone)]
pub struct RelatedMessage {
    pub origin: MessageOrigin,
    pub message: MessageView,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageOrigin {
    TxLevel {
        carrier_witness_index: usize,
    },
    Otx {
        witness_index: usize,
        otx_index: usize,
        layout: OtxMessageLayout,
        relation: OtxTypeRelation,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OtxMessageLayout {
    pub base_inputs: Range,
    pub append_inputs: Range,
    pub base_outputs: Range,
    pub append_outputs: Range,
    pub base_cell_deps: Range,
    pub append_cell_deps: Range,
    pub base_header_deps: Range,
    pub append_header_deps: Range,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OtxTypeRelation {
    pub input_type_in_base: bool,
    pub input_type_in_append: bool,
    pub output_type_in_base: bool,
    pub output_type_in_base_covered: bool,
    pub output_type_in_append: bool,
}
