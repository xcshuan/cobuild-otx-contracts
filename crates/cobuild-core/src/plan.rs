use alloc::vec::Vec;

use crate::{layout::Range, view::ActionView};

#[derive(Clone)]
pub struct LockValidationPlan {
    pub lock_script_hash: [u8; 32],
    pub required_signatures: Vec<SigningRequirement>,
    pub related_actions: Vec<RelatedAction>,
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
    pub related_actions: Vec<TypeRelatedAction>,
}

#[derive(Clone)]
pub struct TypeRelatedAction {
    pub action: RelatedAction,
    pub otx_type_scope: TypeActionOtxScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeActionOtxScope {
    TargetOnly,
    InOtxScope(OtxTypeRelation),
}

impl TypeActionOtxScope {
    pub fn in_otx_scope(self) -> Option<OtxTypeRelation> {
        match self {
            Self::TargetOnly => None,
            Self::InOtxScope(relation) => Some(relation),
        }
    }
}

#[derive(Clone)]
pub struct RelatedAction {
    pub origin: ActionOrigin,
    pub action: ActionView,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionOrigin {
    TxLevel {
        witness_index: usize,
    },
    Otx {
        witness_index: usize,
        otx_index: usize,
        layout: OtxMessageLayout,
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
