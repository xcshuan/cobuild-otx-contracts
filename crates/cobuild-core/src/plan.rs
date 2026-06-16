use alloc::vec::Vec;
use core::cmp::Ordering;

use blake2b_ref::Blake2bBuilder;

use crate::{
    error::CoreError,
    hash::checked_len_prefix,
    layout::{IndexRange, Range},
    protocol::ScriptRole,
    reader::update_cursor_with_error,
    view::ActionView,
};

const ACTION_HASH_PERSONAL: &[u8; 16] = b"ckbcb_act_core1\0";

#[derive(Clone)]
pub struct LockValidationPlan {
    pub lock_script_hash: [u8; 32],
    pub required_signatures: Vec<SigningRequirement>,
    pub related_actions: Vec<RelatedAction>,
}

impl LockValidationPlan {
    pub fn unique_related_action(&self) -> Result<Option<&RelatedAction>, CoreError> {
        unique_slice_item(&self.related_actions)
    }
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

impl TypeValidationPlan {
    pub fn unique_related_action(&self) -> Result<Option<&TypeRelatedAction>, CoreError> {
        unique_slice_item(&self.related_actions)
    }
}

#[derive(Clone)]
pub struct TypeRelatedAction {
    pub action: RelatedAction,
    pub otx_type_scope: TypeActionOtxScope,
}

impl TypeRelatedAction {
    pub fn action_ref(&self) -> ActionRef {
        self.action.action_ref()
    }
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

impl RelatedAction {
    pub fn action_ref(&self) -> ActionRef {
        self.origin.action_ref(self.action.index)
    }

    pub fn action_hash(&self) -> Result<[u8; 32], CoreError> {
        action_hash(self.action_ref(), &self.action)
    }
}

pub fn action_hash(action_ref: ActionRef, action: &ActionView) -> Result<[u8; 32], CoreError> {
    let mut hasher = Blake2bBuilder::new(32)
        .personal(ACTION_HASH_PERSONAL)
        .build();

    write_action_ref(&mut hasher, action_ref)?;
    hasher.update(&action.script_info_hash);
    hasher.update(&[script_role_raw(action.script_role)]);
    hasher.update(&action.script_hash);
    hasher.update(&checked_len_prefix(action.data.size)?);
    update_cursor_with_error(&mut hasher, &action.data, CoreError::MalformedCobuild)?;

    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    Ok(out)
}

fn write_action_ref(
    hasher: &mut blake2b_ref::Blake2b,
    action_ref: ActionRef,
) -> Result<(), CoreError> {
    match action_ref {
        ActionRef::TxLevel {
            witness_index,
            action_index,
        } => {
            hasher.update(&[0]);
            hasher.update(&checked_len_prefix(witness_index)?);
            hasher.update(&[0; 4]);
            hasher.update(&checked_len_prefix(action_index)?);
        }
        ActionRef::Otx {
            witness_index,
            otx_index,
            action_index,
        } => {
            hasher.update(&[1]);
            hasher.update(&checked_len_prefix(witness_index)?);
            hasher.update(&checked_len_prefix(otx_index)?);
            hasher.update(&checked_len_prefix(action_index)?);
        }
    }
    Ok(())
}

fn script_role_raw(role: ScriptRole) -> u8 {
    match role {
        ScriptRole::InputLock => 0,
        ScriptRole::InputType => 1,
        ScriptRole::OutputType => 2,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionRef {
    TxLevel {
        witness_index: usize,
        action_index: usize,
    },
    Otx {
        witness_index: usize,
        otx_index: usize,
        action_index: usize,
    },
}

impl ActionRef {
    fn sort_key(self) -> (usize, u8, usize, usize) {
        match self {
            Self::TxLevel {
                witness_index,
                action_index,
            } => (witness_index, 0, 0, action_index),
            Self::Otx {
                witness_index,
                otx_index,
                action_index,
            } => (witness_index, 1, otx_index, action_index),
        }
    }
}

impl Ord for ActionRef {
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort_key().cmp(&other.sort_key())
    }
}

impl PartialOrd for ActionRef {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
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

impl ActionOrigin {
    pub fn witness_index(self) -> usize {
        match self {
            Self::TxLevel { witness_index } | Self::Otx { witness_index, .. } => witness_index,
        }
    }

    pub fn otx_index(self) -> Option<usize> {
        match self {
            Self::TxLevel { .. } => None,
            Self::Otx { otx_index, .. } => Some(otx_index),
        }
    }

    pub fn otx_layout(self) -> Option<OtxMessageLayout> {
        match self {
            Self::TxLevel { .. } => None,
            Self::Otx { layout, .. } => Some(layout),
        }
    }

    pub fn is_tx_level(self) -> bool {
        matches!(self, Self::TxLevel { .. })
    }

    pub fn is_otx(self) -> bool {
        matches!(self, Self::Otx { .. })
    }

    pub fn action_ref(self, action_index: usize) -> ActionRef {
        match self {
            Self::TxLevel { witness_index } => ActionRef::TxLevel {
                witness_index,
                action_index,
            },
            Self::Otx {
                witness_index,
                otx_index,
                ..
            } => ActionRef::Otx {
                witness_index,
                otx_index,
                action_index,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OtxPart {
    Base,
    Append,
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

    pub fn contains_input(&self, index: usize) -> bool {
        self.classify_input(index).is_some()
    }

    pub fn classify_input(&self, index: usize) -> Option<(OtxPart, usize)> {
        classify_index(self.base_inputs, self.append_inputs, index)
    }

    pub fn outputs(&self) -> Range {
        merge_adjacent_ranges(self.base_outputs, self.append_outputs)
    }

    pub fn output_indexes(&self) -> IndexRange {
        self.outputs().indexes()
    }

    pub fn contains_output(&self, index: usize) -> bool {
        self.classify_output(index).is_some()
    }

    pub fn classify_output(&self, index: usize) -> Option<(OtxPart, usize)> {
        classify_index(self.base_outputs, self.append_outputs, index)
    }

    pub fn cell_deps(&self) -> Range {
        merge_adjacent_ranges(self.base_cell_deps, self.append_cell_deps)
    }

    pub fn cell_dep_indexes(&self) -> IndexRange {
        self.cell_deps().indexes()
    }

    pub fn contains_cell_dep(&self, index: usize) -> bool {
        self.classify_cell_dep(index).is_some()
    }

    pub fn classify_cell_dep(&self, index: usize) -> Option<(OtxPart, usize)> {
        classify_index(self.base_cell_deps, self.append_cell_deps, index)
    }

    pub fn header_deps(&self) -> Range {
        merge_adjacent_ranges(self.base_header_deps, self.append_header_deps)
    }

    pub fn header_dep_indexes(&self) -> IndexRange {
        self.header_deps().indexes()
    }

    pub fn contains_header_dep(&self, index: usize) -> bool {
        self.classify_header_dep(index).is_some()
    }

    pub fn classify_header_dep(&self, index: usize) -> Option<(OtxPart, usize)> {
        classify_index(self.base_header_deps, self.append_header_deps, index)
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

fn classify_index(base: Range, append: Range, index: usize) -> Option<(OtxPart, usize)> {
    if let Some(local_index) = base.local_index(index) {
        return Some((OtxPart::Base, local_index));
    }
    append
        .local_index(index)
        .map(|local_index| (OtxPart::Append, local_index))
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

impl OtxTypeRelation {
    pub fn input_type_in_base(self) -> bool {
        self.input_type_in_base
    }

    pub fn input_type_in_append(self) -> bool {
        self.input_type_in_append
    }

    pub fn output_type_in_base(self) -> bool {
        self.output_type_in_base
    }

    pub fn output_type_in_base_covered(self) -> bool {
        self.output_type_in_base_covered
    }

    pub fn output_type_in_append(self) -> bool {
        self.output_type_in_append
    }

    pub fn input_type_present(self) -> bool {
        self.input_type_in_base || self.input_type_in_append
    }

    pub fn output_type_present(self) -> bool {
        self.output_type_in_base || self.output_type_in_append
    }

    pub fn base_type_present(self) -> bool {
        self.input_type_in_base || self.output_type_in_base
    }

    pub fn append_type_present(self) -> bool {
        self.input_type_in_append || self.output_type_in_append
    }

    pub fn type_present(self) -> bool {
        self.input_type_present() || self.output_type_present()
    }
}

fn unique_slice_item<T>(items: &[T]) -> Result<Option<&T>, CoreError> {
    match items {
        [] => Ok(None),
        [item] => Ok(Some(item)),
        _ => Err(CoreError::DuplicateMatchingAction),
    }
}
