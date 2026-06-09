use alloc::vec::Vec;

use crate::{
    layout::{IndexRange, Range},
    view::ActionView,
};

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

impl OtxMessageLayout {
    pub fn inputs(&self) -> Range {
        merge_adjacent_ranges(self.base_inputs, self.append_inputs)
    }

    pub fn input_indexes(&self) -> IndexRange {
        self.inputs().indexes()
    }

    pub fn outputs(&self) -> Range {
        merge_adjacent_ranges(self.base_outputs, self.append_outputs)
    }

    pub fn output_indexes(&self) -> IndexRange {
        self.outputs().indexes()
    }

    pub fn cell_deps(&self) -> Range {
        merge_adjacent_ranges(self.base_cell_deps, self.append_cell_deps)
    }

    pub fn cell_dep_indexes(&self) -> IndexRange {
        self.cell_deps().indexes()
    }

    pub fn header_deps(&self) -> Range {
        merge_adjacent_ranges(self.base_header_deps, self.append_header_deps)
    }

    pub fn header_dep_indexes(&self) -> IndexRange {
        self.header_deps().indexes()
    }

    pub fn base_inputs(&self) -> Range {
        self.base_inputs
    }

    pub fn append_inputs(&self) -> Range {
        self.append_inputs
    }

    pub fn base_outputs(&self) -> Range {
        self.base_outputs
    }

    pub fn append_outputs(&self) -> Range {
        self.append_outputs
    }

    pub fn base_cell_deps(&self) -> Range {
        self.base_cell_deps
    }

    pub fn append_cell_deps(&self) -> Range {
        self.append_cell_deps
    }

    pub fn base_header_deps(&self) -> Range {
        self.base_header_deps
    }

    pub fn append_header_deps(&self) -> Range {
        self.append_header_deps
    }
}

fn merge_adjacent_ranges(base: Range, append: Range) -> Range {
    debug_assert_eq!(base.start.checked_add(base.count), Some(append.start));
    Range {
        start: base.start,
        count: base
            .count
            .checked_add(append.count)
            .expect("valid cobuild layout range"),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OtxTypeRelation {
    pub input_type_in_base: bool,
    pub input_type_in_append: bool,
    pub output_type_in_base: bool,
    pub output_type_in_base_covered: bool,
    pub output_type_in_append: bool,
}
