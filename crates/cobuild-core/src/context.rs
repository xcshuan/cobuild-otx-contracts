use alloc::{collections::BTreeSet, vec::Vec};

#[cfg(test)]
use cobuild_types::lazy_reader::support::Cursor;

use crate::{
    error::CoreError,
    layout::{OtxLayoutEntry, Range},
    plan::OtxTypeRelation,
    protocol::ScriptRole,
    syscalls::SyscallTxReader,
    view::ActionView,
};

#[cfg(test)]
use crate::view::MessageView;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentScript {
    InputLock([u8; 32]),
    Type([u8; 32]),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentScriptContext {
    current_script: CurrentScript,
    indices: CurrentScriptIndices,
    script_hashes: ScriptHashes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CurrentScriptIndices {
    Lock {
        input_indices: Vec<usize>,
    },
    Type {
        input_indices: Vec<usize>,
        output_indices: Vec<usize>,
    },
}

impl CurrentScriptIndices {
    fn from_script(current_script: CurrentScript) -> Self {
        match current_script {
            CurrentScript::InputLock(_) => Self::Lock {
                input_indices: Vec::new(),
            },
            CurrentScript::Type(_) => Self::Type {
                input_indices: Vec::new(),
                output_indices: Vec::new(),
            },
        }
    }

    fn push_input(&mut self, index: usize) -> Result<(), CoreError> {
        match self {
            Self::Lock { input_indices } | Self::Type { input_indices, .. } => {
                input_indices.push(index);
                Ok(())
            }
        }
    }

    fn push_output(&mut self, index: usize) -> Result<(), CoreError> {
        match self {
            Self::Type { output_indices, .. } => {
                output_indices.push(index);
                Ok(())
            }
            Self::Lock { .. } => Err(CoreError::InvalidContextInput),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ScriptHashes {
    input_locks: BTreeSet<[u8; 32]>,
    input_types: BTreeSet<[u8; 32]>,
    output_types: BTreeSet<[u8; 32]>,
}

impl CurrentScriptContext {
    #[cfg(test)]
    pub(crate) fn from_script_for_tests(current_script: CurrentScript) -> Self {
        Self {
            current_script,
            indices: CurrentScriptIndices::from_script(current_script),
            script_hashes: ScriptHashes::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn input_lock_for_tests(
        current_lock_hash: [u8; 32],
        input_locks: Vec<[u8; 32]>,
    ) -> Self {
        let mut context = Self::from_script_for_tests(CurrentScript::InputLock(current_lock_hash));
        for (index, lock_hash) in input_locks.into_iter().enumerate() {
            context.push_input_lock_hash(index, lock_hash).unwrap();
        }
        context
    }

    pub(crate) fn from_reader(
        reader: &SyscallTxReader,
        current_script: CurrentScript,
    ) -> Result<Self, CoreError> {
        let counts = reader.counts();
        let mut context = Self {
            current_script,
            indices: CurrentScriptIndices::from_script(current_script),
            script_hashes: ScriptHashes::default(),
        };

        for index in 0..counts.inputs {
            let lock_hash = reader.input_lock_hash(index)?;
            context.push_input_lock_hash(index, lock_hash)?;

            if let Some(type_hash) = reader.input_type_hash(index)? {
                context.push_input_type_hash(index, type_hash)?;
            }
        }

        for index in 0..counts.outputs {
            if let Some(type_hash) = reader.output_type_hash(index)? {
                context.push_output_type_hash(index, type_hash)?;
            }
        }

        Ok(context)
    }

    fn push_input_lock_hash(&mut self, index: usize, lock_hash: [u8; 32]) -> Result<(), CoreError> {
        self.script_hashes.input_locks.insert(lock_hash);
        if self.current_script == CurrentScript::InputLock(lock_hash) {
            self.indices.push_input(index)?;
        }

        Ok(())
    }

    fn push_input_type_hash(&mut self, index: usize, type_hash: [u8; 32]) -> Result<(), CoreError> {
        self.script_hashes.input_types.insert(type_hash);
        if self.current_script == CurrentScript::Type(type_hash) {
            self.indices.push_input(index)?;
        }

        Ok(())
    }

    fn push_output_type_hash(
        &mut self,
        index: usize,
        type_hash: [u8; 32],
    ) -> Result<(), CoreError> {
        self.script_hashes.output_types.insert(type_hash);
        if self.current_script == CurrentScript::Type(type_hash) {
            self.indices.push_output(index)?;
        }

        Ok(())
    }

    pub(crate) fn current_lock_hash(&self) -> Result<[u8; 32], CoreError> {
        match self.current_script {
            CurrentScript::InputLock(lock_hash) => Ok(lock_hash),
            CurrentScript::Type(_) => Err(CoreError::InvalidContextInput),
        }
    }

    pub(crate) fn current_type_hash(&self) -> Result<[u8; 32], CoreError> {
        match self.current_script {
            CurrentScript::Type(type_hash) => Ok(type_hash),
            CurrentScript::InputLock(_) => Err(CoreError::InvalidContextInput),
        }
    }

    pub(crate) fn current_lock_inputs(&self) -> Result<&[usize], CoreError> {
        match &self.indices {
            CurrentScriptIndices::Lock { input_indices } => Ok(input_indices),
            CurrentScriptIndices::Type { .. } => Err(CoreError::InvalidContextInput),
        }
    }

    pub(crate) fn type_input_indices(&self) -> Result<&[usize], CoreError> {
        match &self.indices {
            CurrentScriptIndices::Type { input_indices, .. } => Ok(input_indices),
            CurrentScriptIndices::Lock { .. } => Err(CoreError::InvalidContextInput),
        }
    }

    pub(crate) fn type_output_indices(&self) -> Result<&[usize], CoreError> {
        match &self.indices {
            CurrentScriptIndices::Type { output_indices, .. } => Ok(output_indices),
            CurrentScriptIndices::Lock { .. } => Err(CoreError::InvalidContextInput),
        }
    }

    pub(crate) fn input_range_contains_current_lock(
        &self,
        range: Range,
    ) -> Result<bool, CoreError> {
        Ok(self
            .current_lock_inputs()?
            .iter()
            .any(|index| range_contains(range, *index)))
    }

    pub(crate) fn type_relation_for_otx(
        &self,
        otx: &OtxLayoutEntry,
    ) -> Result<OtxTypeRelation, CoreError> {
        Ok(OtxTypeRelation {
            input_type_in_base: self.type_in_input_range(otx.layout.base_inputs)?,
            input_type_in_append: self.type_in_input_range(otx.layout.append_inputs)?,
            output_type_in_base: self.type_in_output_range(otx.layout.base_outputs)?,
            output_type_in_base_covered: self.covered_current_type_in_base_outputs(otx)?,
            output_type_in_append: self.type_in_output_range(otx.layout.append_outputs)?,
        })
    }

    pub(crate) fn current_type_present(&self) -> Result<bool, CoreError> {
        Ok(!self.type_input_indices()?.is_empty() || !self.type_output_indices()?.is_empty())
    }

    pub(crate) fn current_type_outside_ranges(
        &self,
        input_range: Range,
        output_range: Range,
    ) -> Result<bool, CoreError> {
        Ok(self
            .type_input_indices()?
            .iter()
            .any(|index| !range_contains(input_range, *index))
            || self
                .type_output_indices()?
                .iter()
                .any(|index| !range_contains(output_range, *index)))
    }

    pub(crate) fn current_lock_has_inputs_outside_range(
        &self,
        range: Range,
    ) -> Result<bool, CoreError> {
        Ok(self
            .current_lock_inputs()?
            .iter()
            .any(|index| !range_contains(range, *index)))
    }

    pub(crate) fn all_current_lock_inputs_in_range(&self, range: Range) -> Result<bool, CoreError> {
        Ok(self
            .current_lock_inputs()?
            .iter()
            .all(|index| range_contains(range, *index)))
    }

    #[cfg(test)]
    pub(crate) fn validate_message_targets(&self, message: &Cursor) -> Result<(), CoreError> {
        let actions = MessageView::new(message.clone()).actions()?;
        self.validate_action_targets(&actions)
    }

    pub(crate) fn validate_action_targets(&self, actions: &[ActionView]) -> Result<(), CoreError> {
        for action in actions {
            if !self
                .script_hashes
                .contains(action.script_role, action.script_hash)
            {
                return Err(CoreError::InvalidMessageTarget);
            }
        }
        Ok(())
    }

    fn type_in_input_range(&self, range: Range) -> Result<bool, CoreError> {
        Ok(self
            .type_input_indices()?
            .iter()
            .any(|index| range_contains(range, *index)))
    }

    fn type_in_output_range(&self, range: Range) -> Result<bool, CoreError> {
        Ok(self
            .type_output_indices()?
            .iter()
            .any(|index| range_contains(range, *index)))
    }

    fn covered_current_type_in_base_outputs(
        &self,
        otx: &OtxLayoutEntry,
    ) -> Result<bool, CoreError> {
        for local_index in 0..otx.layout.base_outputs.count {
            let tx_index = otx
                .layout
                .base_outputs
                .start
                .checked_add(local_index)
                .ok_or(CoreError::InvalidOtxLayout)?;
            if !self.type_output_indices()?.contains(&tx_index) {
                continue;
            }
            if otx.witness.includes_base_output_type(local_index)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl ScriptHashes {
    fn contains(&self, role: ScriptRole, script_hash: [u8; 32]) -> bool {
        match role {
            ScriptRole::InputLock => self.input_locks.contains(&script_hash),
            ScriptRole::InputType => self.input_types.contains(&script_hash),
            ScriptRole::OutputType => self.output_types.contains(&script_hash),
        }
    }
}

fn range_contains(range: Range, index: usize) -> bool {
    index >= range.start && index < range.start.saturating_add(range.count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn range(start: usize, count: usize) -> Range {
        Range { start, count }
    }

    fn message_with_action(script_role: u8, script_hash: [u8; 32]) -> Cursor {
        let action = table_bytes(&[
            hash(0).to_vec(),
            alloc::vec![script_role],
            script_hash.to_vec(),
            molecule_bytes(&[]),
        ]);
        let actions = dynvec_bytes(&[action]);
        let message = table_bytes(&[actions]);
        crate::reader::cursor_from_slice(&message)
    }

    fn molecule_bytes(raw: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(4 + raw.len());
        bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        bytes.extend_from_slice(raw);
        bytes
    }

    fn dynvec_bytes(items: &[Vec<u8>]) -> Vec<u8> {
        if items.is_empty() {
            return 4u32.to_le_bytes().to_vec();
        }

        let header_size = 4 + items.len() * 4;
        let total_size = header_size + items.iter().map(Vec::len).sum::<usize>();
        let mut bytes = Vec::with_capacity(total_size);
        bytes.extend_from_slice(&(total_size as u32).to_le_bytes());

        let mut offset = header_size;
        for item in items {
            bytes.extend_from_slice(&(offset as u32).to_le_bytes());
            offset += item.len();
        }
        for item in items {
            bytes.extend_from_slice(item);
        }

        bytes
    }

    fn table_bytes(fields: &[Vec<u8>]) -> Vec<u8> {
        let header_size = 4 + fields.len() * 4;
        let total_size = header_size + fields.iter().map(Vec::len).sum::<usize>();
        let mut bytes = Vec::with_capacity(total_size);
        bytes.extend_from_slice(&(total_size as u32).to_le_bytes());

        let mut offset = header_size;
        for field in fields {
            bytes.extend_from_slice(&(offset as u32).to_le_bytes());
            offset += field.len();
        }
        for field in fields {
            bytes.extend_from_slice(field);
        }

        bytes
    }

    #[test]
    fn current_lock_context_tracks_only_current_lock_indices() {
        let lock_a = hash(1);
        let lock_b = hash(2);
        let context = context_with_scripts(
            CurrentScript::InputLock(lock_a),
            alloc::vec![lock_a, lock_b, lock_a],
            alloc::vec![None, None, None],
            alloc::vec![],
        );

        assert_eq!(context.current_lock_inputs(), Ok([0, 2].as_slice()));
        assert_eq!(
            context.input_range_contains_current_lock(range(1, 1)),
            Ok(false)
        );
        assert_eq!(
            context.input_range_contains_current_lock(range(2, 1)),
            Ok(true)
        );
    }

    #[test]
    fn current_type_context_tracks_current_type_inputs_and_outputs() {
        let type_a = hash(1);
        let type_b = hash(2);
        let context = context_with_scripts(
            CurrentScript::Type(type_a),
            alloc::vec![hash(9), hash(9)],
            alloc::vec![Some(type_a), Some(type_b)],
            alloc::vec![None, Some(type_a)],
        );

        assert_eq!(context.type_input_indices(), Ok([0].as_slice()));
        assert_eq!(context.type_output_indices(), Ok([1].as_slice()));
        assert_eq!(context.current_type_present(), Ok(true));
    }

    #[test]
    fn lock_coverage_uses_current_lock_indices() {
        let lock_a = hash(1);
        let lock_b = hash(2);
        let context = context_with_scripts(
            CurrentScript::InputLock(lock_a),
            alloc::vec![lock_a, lock_b, lock_a],
            alloc::vec![None, None, None],
            alloc::vec![],
        );

        assert_eq!(
            context.all_current_lock_inputs_in_range(range(0, 3)),
            Ok(true)
        );
        assert_eq!(
            context.all_current_lock_inputs_in_range(range(0, 1)),
            Ok(false)
        );
    }

    #[test]
    fn current_lock_has_inputs_outside_range_uses_only_current_lock_indices() {
        let lock_a = hash(1);
        let lock_b = hash(2);
        let context = context_with_scripts(
            CurrentScript::InputLock(lock_a),
            alloc::vec![lock_b, lock_b, lock_a],
            alloc::vec![None, None, None],
            alloc::vec![],
        );

        assert_eq!(
            context.current_lock_has_inputs_outside_range(range(0, 2)),
            Ok(true)
        );
    }

    #[test]
    fn validate_message_targets_accepts_existing_targets() {
        let lock_hash = hash(1);
        let input_type_hash = hash(2);
        let output_type_hash = hash(3);
        let context = context_with_scripts(
            CurrentScript::InputLock(lock_hash),
            alloc::vec![lock_hash],
            alloc::vec![Some(input_type_hash)],
            alloc::vec![Some(output_type_hash)],
        );
        assert!(context
            .validate_message_targets(&message_with_action(0, lock_hash))
            .is_ok());
        assert!(context
            .validate_message_targets(&message_with_action(1, input_type_hash))
            .is_ok());
        assert!(context
            .validate_message_targets(&message_with_action(2, output_type_hash))
            .is_ok());
    }

    #[test]
    fn validate_message_targets_rejects_missing_or_unknown_targets() {
        let context = context_with_scripts(
            CurrentScript::InputLock(hash(1)),
            alloc::vec![hash(1)],
            alloc::vec![None],
            alloc::vec![],
        );
        for script_role in [0, 1, 2, 9] {
            assert_eq!(
                context.validate_message_targets(&message_with_action(script_role, hash(7))),
                Err(CoreError::InvalidMessageTarget)
            );
        }
    }

    #[test]
    fn script_hashes_cover_full_transaction_for_target_validation() {
        let lock_hash = hash(1);
        let other_lock_hash = hash(2);
        let input_type_hash = hash(3);
        let output_type_hash = hash(4);
        let context = context_with_scripts(
            CurrentScript::InputLock(lock_hash),
            alloc::vec![other_lock_hash, lock_hash, lock_hash],
            alloc::vec![Some(input_type_hash), Some(input_type_hash), None],
            alloc::vec![Some(output_type_hash), Some(output_type_hash)],
        );

        assert_eq!(context.current_lock_inputs(), Ok([1, 2].as_slice()));
        assert_eq!(context.script_hashes.input_locks.len(), 2);
        assert_eq!(context.script_hashes.input_types.len(), 1);
        assert_eq!(context.script_hashes.output_types.len(), 1);
        assert!(context
            .validate_message_targets(&message_with_action(0, lock_hash))
            .is_ok());
        assert!(context
            .validate_message_targets(&message_with_action(1, input_type_hash))
            .is_ok());
        assert!(context
            .validate_message_targets(&message_with_action(2, output_type_hash))
            .is_ok());
    }

    #[test]
    fn type_relation_tracks_base_append_inputs_and_outputs() {
        let type_hash = hash(1);
        let context = context_with_scripts(
            CurrentScript::Type(type_hash),
            alloc::vec![hash(9), hash(9), hash(9)],
            alloc::vec![Some(type_hash), Some(type_hash), None],
            alloc::vec![Some(type_hash), Some(type_hash), None],
        );
        let otx = otx_entry_for_type_relation(
            range(0, 1),
            range(1, 1),
            range(0, 1),
            range(1, 1),
            &[0b0000_0100],
        );

        let relation = context.type_relation_for_otx(&otx).unwrap();

        assert!(relation.input_type_in_base);
        assert!(relation.input_type_in_append);
        assert!(relation.output_type_in_base);
        assert!(relation.output_type_in_base_covered);
        assert!(relation.output_type_in_append);
    }

    #[test]
    fn type_relation_reports_uncovered_base_output_type() {
        let type_hash = hash(1);
        let context = context_with_scripts(
            CurrentScript::Type(type_hash),
            alloc::vec![hash(9)],
            alloc::vec![None],
            alloc::vec![Some(type_hash)],
        );
        let otx = otx_entry_for_type_relation(
            range(0, 1),
            range(1, 0),
            range(0, 1),
            range(1, 0),
            &[0b0000_0000],
        );

        let relation = context.type_relation_for_otx(&otx).unwrap();

        assert!(!relation.input_type_in_base);
        assert!(!relation.input_type_in_append);
        assert!(relation.output_type_in_base);
        assert!(!relation.output_type_in_base_covered);
        assert!(!relation.output_type_in_append);
    }

    #[test]
    fn type_relation_ignores_current_type_outside_otx_ranges() {
        let type_hash = hash(1);
        let context = context_with_scripts(
            CurrentScript::Type(type_hash),
            alloc::vec![hash(9), hash(9), hash(9)],
            alloc::vec![None, None, Some(type_hash)],
            alloc::vec![None, None, Some(type_hash)],
        );
        let otx = otx_entry_for_type_relation(
            range(0, 1),
            range(1, 1),
            range(0, 1),
            range(1, 1),
            &[0b0000_0100],
        );

        let relation = context.type_relation_for_otx(&otx).unwrap();

        assert!(!relation.input_type_in_base);
        assert!(!relation.input_type_in_append);
        assert!(!relation.output_type_in_base);
        assert!(!relation.output_type_in_base_covered);
        assert!(!relation.output_type_in_append);
    }

    fn otx_entry_for_type_relation(
        base_inputs: Range,
        append_inputs: Range,
        base_outputs: Range,
        append_outputs: Range,
        base_output_masks: &[u8],
    ) -> crate::layout::OtxLayoutEntry {
        crate::layout::OtxLayoutEntry {
            layout: crate::layout::OtxLayout {
                witness_index: 0,
                base_inputs,
                append_inputs,
                base_outputs,
                append_outputs,
                base_cell_deps: range(0, 0),
                append_cell_deps: range(0, 0),
                base_header_deps: range(0, 0),
                append_header_deps: range(0, 0),
                append_segments: alloc::vec![crate::layout::OtxAppendSegmentLayout {
                    flags: crate::protocol::SegmentFlags::try_from(0).unwrap(),
                    inputs: append_inputs,
                    outputs: append_outputs,
                    cell_deps: range(0, 0),
                    header_deps: range(0, 0),
                }],
            },
            witness: crate::view::OtxView {
                message: crate::reader::cursor_from_slice(&empty_message()),
                append_permissions: 0,
                base_input_cells: base_inputs.count,
                base_input_masks: crate::view::MaskView::new(alloc::vec![0]),
                base_output_cells: base_outputs.count,
                base_output_masks: crate::view::MaskView::new(base_output_masks.to_vec()),
                base_cell_deps: 0,
                base_cell_dep_masks: crate::view::MaskView::new(Vec::new()),
                base_header_deps: 0,
                base_header_dep_masks: crate::view::MaskView::new(Vec::new()),
                append_segments: alloc::vec![crate::view::OtxAppendSegmentView {
                    segment_flags: 0,
                    input_cells: append_inputs.count,
                    output_cells: append_outputs.count,
                    cell_deps: 0,
                    header_deps: 0,
                    seals: Vec::new(),
                }],
                base_seals: Vec::new(),
            },
        }
    }

    fn empty_message() -> Vec<u8> {
        table_bytes(&[dynvec_bytes(&[])])
    }

    fn context_with_scripts(
        current_script: CurrentScript,
        input_locks: Vec<[u8; 32]>,
        input_types: Vec<Option<[u8; 32]>>,
        output_types: Vec<Option<[u8; 32]>>,
    ) -> CurrentScriptContext {
        assert_eq!(input_locks.len(), input_types.len());
        let mut context = CurrentScriptContext {
            current_script,
            indices: CurrentScriptIndices::from_script(current_script),
            script_hashes: ScriptHashes::default(),
        };

        for (index, (lock_hash, type_hash)) in input_locks.into_iter().zip(input_types).enumerate()
        {
            context.push_input_lock_hash(index, lock_hash).unwrap();
            if let Some(type_hash) = type_hash {
                context.push_input_type_hash(index, type_hash).unwrap();
            }
        }

        for (index, type_hash) in output_types.into_iter().enumerate() {
            if let Some(type_hash) = type_hash {
                context.push_output_type_hash(index, type_hash).unwrap();
            }
        }

        context
    }
}
