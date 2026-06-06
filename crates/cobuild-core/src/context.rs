use alloc::vec::Vec;

use cobuild_types::lazy_reader::support::Cursor;

use crate::{
    error::CoreError,
    layout::{OtxLayoutEntry, Range},
    plan::OtxTypeRelation,
    protocol::ScriptRole,
    syscalls::SyscallTxReader,
    view::MessageView,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentScript {
    InputLock([u8; 32]),
    Type([u8; 32]),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentScriptContext {
    current_script: CurrentScript,
    indices: CurrentScriptIndices,
    #[cfg(test)]
    target_hashes_for_tests: Option<TargetHashesForTests>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
enum CurrentScriptIndices {
    #[default]
    Empty,
    Lock {
        input_indices: Vec<usize>,
    },
    Type {
        input_indices: Vec<usize>,
        output_indices: Vec<usize>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TargetPresence {
    role: ScriptRole,
    script_hash: [u8; 32],
    exists: bool,
}

#[cfg(test)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TargetHashesForTests {
    input_lock_hashes: Vec<[u8; 32]>,
    input_type_hashes: Vec<[u8; 32]>,
    output_type_hashes: Vec<[u8; 32]>,
}

impl CurrentScriptContext {
    pub(crate) fn from_reader(
        reader: &SyscallTxReader,
        current_script: CurrentScript,
    ) -> Result<Self, CoreError> {
        let counts = reader.counts();
        let indices = match current_script {
            CurrentScript::InputLock(lock_hash) => {
                let mut input_indices = Vec::new();
                for index in 0..counts.inputs {
                    if reader.input_lock_hash(index)? == lock_hash {
                        input_indices.push(index);
                    }
                }
                CurrentScriptIndices::Lock { input_indices }
            }
            CurrentScript::Type(type_hash) => {
                let mut input_indices = Vec::new();
                for index in 0..counts.inputs {
                    if reader.input_type_hash(index)? == Some(type_hash) {
                        input_indices.push(index);
                    }
                }

                let mut output_indices = Vec::new();
                for index in 0..counts.outputs {
                    if reader.output_type_hash(index)? == Some(type_hash) {
                        output_indices.push(index);
                    }
                }
                CurrentScriptIndices::Type {
                    input_indices,
                    output_indices,
                }
            }
        };

        Ok(Self {
            current_script,
            indices,
            #[cfg(test)]
            target_hashes_for_tests: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_parts_for_tests(
        current_script: CurrentScript,
        input_locks: Vec<[u8; 32]>,
        input_types: Vec<Option<[u8; 32]>>,
        output_types: Vec<Option<[u8; 32]>>,
    ) -> Self {
        let indices = match current_script {
            CurrentScript::InputLock(lock_hash) => CurrentScriptIndices::Lock {
                input_indices: input_locks
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| (*candidate == lock_hash).then_some(index))
                    .collect(),
            },
            CurrentScript::Type(type_hash) => CurrentScriptIndices::Type {
                input_indices: input_types
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| {
                        (*candidate == Some(type_hash)).then_some(index)
                    })
                    .collect(),
                output_indices: output_types
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| {
                        (*candidate == Some(type_hash)).then_some(index)
                    })
                    .collect(),
            },
        };
        let target_hashes_for_tests = TargetHashesForTests {
            input_lock_hashes: input_locks,
            input_type_hashes: input_types.into_iter().flatten().collect(),
            output_type_hashes: output_types.into_iter().flatten().collect(),
        };

        Self {
            current_script,
            indices,
            target_hashes_for_tests: Some(target_hashes_for_tests),
        }
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
            CurrentScriptIndices::Empty | CurrentScriptIndices::Type { .. } => {
                Err(CoreError::InvalidContextInput)
            }
        }
    }

    pub(crate) fn type_input_indices(&self) -> Result<&[usize], CoreError> {
        match &self.indices {
            CurrentScriptIndices::Type { input_indices, .. } => Ok(input_indices),
            CurrentScriptIndices::Empty | CurrentScriptIndices::Lock { .. } => {
                Err(CoreError::InvalidContextInput)
            }
        }
    }

    pub(crate) fn type_output_indices(&self) -> Result<&[usize], CoreError> {
        match &self.indices {
            CurrentScriptIndices::Type { output_indices, .. } => Ok(output_indices),
            CurrentScriptIndices::Empty | CurrentScriptIndices::Lock { .. } => {
                Err(CoreError::InvalidContextInput)
            }
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

    pub(crate) fn current_type_outside_otx_ranges(
        &self,
        otxs: &[OtxLayoutEntry],
    ) -> Result<bool, CoreError> {
        Ok(self.type_input_indices()?.iter().any(|index| {
            !otxs.iter().any(|entry| {
                range_contains(entry.layout.base_inputs, *index)
                    || range_contains(entry.layout.append_inputs, *index)
            })
        }) || self.type_output_indices()?.iter().any(|index| {
            !otxs.iter().any(|entry| {
                range_contains(entry.layout.base_outputs, *index)
                    || range_contains(entry.layout.append_outputs, *index)
            })
        }))
    }

    pub(crate) fn current_lock_outside_otx_ranges(
        &self,
        otxs: &[OtxLayoutEntry],
    ) -> Result<bool, CoreError> {
        Ok(self.current_lock_inputs()?.iter().any(|index| {
            !otxs.iter().any(|entry| {
                range_contains(entry.layout.base_inputs, *index)
                    || range_contains(entry.layout.append_inputs, *index)
            })
        }))
    }

    pub(crate) fn all_current_lock_inputs_covered_by_otx(
        &self,
        otxs: &[OtxLayoutEntry],
    ) -> Result<bool, CoreError> {
        Ok(self.current_lock_inputs()?.iter().all(|index| {
            otxs.iter().any(|entry| {
                range_contains(entry.layout.base_inputs, *index)
                    || range_contains(entry.layout.append_inputs, *index)
            })
        }))
    }

    pub(crate) fn validate_message_targets(
        &self,
        tx: &SyscallTxReader,
        message: &Cursor,
    ) -> Result<(), CoreError> {
        let mut cache = Vec::new();
        for action in MessageView::new(message.clone()).actions()? {
            let target_exists =
                self.target_exists(tx, action.script_role, action.script_hash, &mut cache)?;
            if !target_exists {
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
            if otx.witness.base_output_masks.get(local_index * 4 + 2)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn target_exists(
        &self,
        tx: &SyscallTxReader,
        role: ScriptRole,
        script_hash: [u8; 32],
        cache: &mut Vec<TargetPresence>,
    ) -> Result<bool, CoreError> {
        if let Some(cached) = cache
            .iter()
            .find(|entry| entry.role == role && entry.script_hash == script_hash)
        {
            return Ok(cached.exists);
        }

        let exists = self.find_target(tx, role, script_hash)?;
        cache.push(TargetPresence {
            role,
            script_hash,
            exists,
        });
        Ok(exists)
    }

    fn find_target(
        &self,
        tx: &SyscallTxReader,
        role: ScriptRole,
        script_hash: [u8; 32],
    ) -> Result<bool, CoreError> {
        #[cfg(test)]
        if let Some(targets) = &self.target_hashes_for_tests {
            return Ok(targets.contains(role, script_hash));
        }

        let counts = tx.counts();
        match role {
            ScriptRole::InputLock => {
                for index in 0..counts.inputs {
                    if tx.input_lock_hash(index)? == script_hash {
                        return Ok(true);
                    }
                }
            }
            ScriptRole::InputType => {
                for index in 0..counts.inputs {
                    if tx.input_type_hash(index)? == Some(script_hash) {
                        return Ok(true);
                    }
                }
            }
            ScriptRole::OutputType => {
                for index in 0..counts.outputs {
                    if tx.output_type_hash(index)? == Some(script_hash) {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
impl TargetHashesForTests {
    fn contains(&self, role: ScriptRole, script_hash: [u8; 32]) -> bool {
        match role {
            ScriptRole::InputLock => self.input_lock_hashes.contains(&script_hash),
            ScriptRole::InputType => self.input_type_hashes.contains(&script_hash),
            ScriptRole::OutputType => self.output_type_hashes.contains(&script_hash),
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

    fn otx_entry(layout: crate::layout::OtxLayout) -> OtxLayoutEntry {
        OtxLayoutEntry {
            layout,
            witness: crate::view::OtxView {
                message: crate::reader::cursor_from_slice(&[4, 0, 0, 0]),
                append_permissions: 0,
                base_input_cells: 1,
                base_input_masks: crate::view::MaskView::new(alloc::vec![0]),
                base_output_cells: 0,
                base_output_masks: crate::view::MaskView::new(Vec::new()),
                base_cell_deps: 0,
                base_cell_dep_masks: crate::view::MaskView::new(Vec::new()),
                base_header_deps: 0,
                base_header_dep_masks: crate::view::MaskView::new(Vec::new()),
                append_input_cells: 0,
                append_output_cells: 0,
                append_cell_deps: 0,
                append_header_deps: 0,
                seals: Vec::new(),
            },
        }
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
        let context = CurrentScriptContext::from_parts_for_tests(
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
        let context = CurrentScriptContext::from_parts_for_tests(
            CurrentScript::Type(type_a),
            alloc::vec![],
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
        let context = CurrentScriptContext::from_parts_for_tests(
            CurrentScript::InputLock(lock_a),
            alloc::vec![lock_a, lock_b, lock_a],
            alloc::vec![None, None, None],
            alloc::vec![],
        );
        let layout_covering_lock_a = otx_entry(crate::layout::OtxLayout {
            witness_index: 0,
            base_inputs: range(0, 1),
            append_inputs: range(2, 1),
            base_outputs: range(0, 0),
            append_outputs: range(0, 0),
            base_cell_deps: range(0, 0),
            append_cell_deps: range(0, 0),
            base_header_deps: range(0, 0),
            append_header_deps: range(0, 0),
        });

        assert_eq!(
            context.all_current_lock_inputs_covered_by_otx(&[layout_covering_lock_a]),
            Ok(true)
        );
        assert_eq!(
            context.all_current_lock_inputs_covered_by_otx(&[]),
            Ok(false)
        );
    }

    #[test]
    fn validate_message_targets_accepts_existing_targets() {
        let lock_hash = hash(1);
        let input_type_hash = hash(2);
        let output_type_hash = hash(3);
        let context = CurrentScriptContext::from_parts_for_tests(
            CurrentScript::InputLock(lock_hash),
            alloc::vec![lock_hash],
            alloc::vec![Some(input_type_hash)],
            alloc::vec![Some(output_type_hash)],
        );
        let tx = SyscallTxReader::from_cached_parts_for_tests(
            crate::syscalls::TxCounts::default(),
            crate::reader::cursor_from_slice(&[4, 0, 0, 0]),
            [0; 32],
        );

        assert!(context
            .validate_message_targets(&tx, &message_with_action(0, lock_hash))
            .is_ok());
        assert!(context
            .validate_message_targets(&tx, &message_with_action(1, input_type_hash))
            .is_ok());
        assert!(context
            .validate_message_targets(&tx, &message_with_action(2, output_type_hash))
            .is_ok());
    }

    #[test]
    fn validate_message_targets_rejects_missing_or_unknown_targets() {
        let context = CurrentScriptContext::from_parts_for_tests(
            CurrentScript::InputLock(hash(1)),
            alloc::vec![hash(1)],
            alloc::vec![],
            alloc::vec![],
        );
        let tx = SyscallTxReader::from_cached_parts_for_tests(
            crate::syscalls::TxCounts::default(),
            crate::reader::cursor_from_slice(&[4, 0, 0, 0]),
            [0; 32],
        );

        for script_role in [0, 1, 2, 9] {
            assert_eq!(
                context.validate_message_targets(&tx, &message_with_action(script_role, hash(7))),
                Err(CoreError::InvalidMessageTarget)
            );
        }
    }
}
