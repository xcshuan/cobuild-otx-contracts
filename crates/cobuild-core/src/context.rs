use alloc::vec::Vec;

use cobuild_types::lazy_reader::support::Cursor;

use crate::{
    error::CoreError,
    layout::{OtxLayoutEntry, Range},
    plan::OtxTypeRelation,
    protocol::ScriptRole,
    reader::cursor_bytes_with_error,
    syscalls::SyscallTxReader,
    view::MessageView,
    witness::WitnessScan,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CurrentLockGroup {
    input_indices: Option<Vec<usize>>,
}

impl CurrentLockGroup {
    pub(crate) fn from_source() -> Self {
        Self {
            input_indices: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_input_indices(input_indices: Vec<usize>) -> Self {
        Self {
            input_indices: Some(input_indices),
        }
    }

    pub(crate) fn carrier_witness_index(
        &self,
        tx: &SyscallTxReader,
    ) -> Result<Option<usize>, CoreError> {
        match &self.input_indices {
            Some(input_indices) => Ok(input_indices.first().copied()),
            None => tx
                .current_lock_group_has_input(0)
                .map(|has_input| has_input.then_some(0)),
        }
    }

    pub(crate) fn carrier_has_sighash_all_layout(
        &self,
        tx: &SyscallTxReader,
        witnesses: &WitnessScan,
    ) -> Result<bool, CoreError> {
        match &self.input_indices {
            Some(input_indices) => {
                let Some(index) = input_indices.first().copied() else {
                    return Ok(false);
                };
                witnesses.tx_level_carrier_has_sighash_all_layout(index)
            }
            None => {
                let Some(witness) = tx.current_lock_group_witness_cursor(0)? else {
                    return Ok(false);
                };
                let witness = cursor_bytes_with_error(&witness, CoreError::MissingHashInput)?;
                WitnessScan::witness_has_sighash_all_layout(&witness)
            }
        }
    }

    pub(crate) fn ensure_non_carrier_witnesses_empty(
        &self,
        tx: &SyscallTxReader,
        witnesses: &WitnessScan,
        carrier_witness_index: usize,
    ) -> Result<(), CoreError> {
        match &self.input_indices {
            Some(input_indices) => witnesses.ensure_non_carrier_witnesses_empty(
                input_indices.iter().copied(),
                carrier_witness_index,
            ),
            None => {
                let mut group_index = 1;
                while tx.current_lock_group_has_input(group_index)? {
                    if let Some(witness) = tx.current_lock_group_witness_cursor(group_index)? {
                        let witness =
                            cursor_bytes_with_error(&witness, CoreError::MissingHashInput)?;
                        if !witness.is_empty() {
                            return Err(CoreError::InvalidLockGroupWitness);
                        }
                    }
                    group_index += 1;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn carrier_witness_bytes(
        &self,
        tx: &SyscallTxReader,
    ) -> Result<Option<Vec<u8>>, CoreError> {
        match &self.input_indices {
            Some(input_indices) => {
                let Some(index) = input_indices.first().copied() else {
                    return Ok(None);
                };
                let witness = tx.witness_cursor(index)?;
                cursor_bytes_with_error(&witness, CoreError::MissingHashInput).map(Some)
            }
            None => {
                let Some(witness) = tx.current_lock_group_witness_cursor(0)? else {
                    return Ok(None);
                };
                cursor_bytes_with_error(&witness, CoreError::MissingHashInput).map(Some)
            }
        }
    }

    pub(crate) fn is_empty(&self, tx: &SyscallTxReader) -> Result<bool, CoreError> {
        self.carrier_witness_index(tx)
            .map(|carrier_witness_index| carrier_witness_index.is_none())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TxScriptHashes {
    pub input_locks: Vec<[u8; 32]>,
    pub input_types: Vec<Option<[u8; 32]>>,
    pub output_types: Vec<Option<[u8; 32]>>,
    lock_input_indices: Vec<LockInputIndices>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct LockInputIndices {
    lock_hash: [u8; 32],
    indices: Vec<usize>,
}

impl TxScriptHashes {
    pub(crate) fn from_reader(reader: &SyscallTxReader) -> Result<Self, CoreError> {
        let counts = reader.counts();
        let mut input_locks = Vec::with_capacity(counts.inputs);
        let mut input_types = Vec::with_capacity(counts.inputs);
        for index in 0..counts.inputs {
            input_locks.push(reader.input_lock_hash(index)?);
            input_types.push(reader.input_type_hash(index)?);
        }

        let mut output_types = Vec::with_capacity(counts.outputs);
        for index in 0..counts.outputs {
            output_types.push(reader.output_type_hash(index)?);
        }

        Ok(Self::from_parts(input_locks, input_types, output_types))
    }

    #[cfg(test)]
    pub(crate) fn from_parts_for_tests(
        input_locks: Vec<[u8; 32]>,
        input_types: Vec<Option<[u8; 32]>>,
        output_types: Vec<Option<[u8; 32]>>,
    ) -> Self {
        Self::from_parts(input_locks, input_types, output_types)
    }

    fn from_parts(
        input_locks: Vec<[u8; 32]>,
        input_types: Vec<Option<[u8; 32]>>,
        output_types: Vec<Option<[u8; 32]>>,
    ) -> Self {
        let lock_input_indices = index_input_locks(&input_locks);
        Self {
            input_locks,
            input_types,
            output_types,
            lock_input_indices,
        }
    }

    pub(crate) fn type_in_input_range(&self, range: Range, type_hash: [u8; 32]) -> bool {
        self.input_types
            .iter()
            .skip(range.start)
            .take(range.count)
            .any(|hash| *hash == Some(type_hash))
    }

    pub(crate) fn input_range_contains_lock(&self, range: Range, lock_hash: [u8; 32]) -> bool {
        self.input_indices_for_lock(lock_hash)
            .iter()
            .any(|index| range_contains(range, *index))
    }

    pub(crate) fn type_in_output_range(&self, range: Range, type_hash: [u8; 32]) -> bool {
        self.output_types
            .iter()
            .skip(range.start)
            .take(range.count)
            .any(|hash| *hash == Some(type_hash))
    }

    pub(crate) fn type_relation_for_otx(
        &self,
        otx: &OtxLayoutEntry,
        type_hash: [u8; 32],
    ) -> Result<OtxTypeRelation, CoreError> {
        Ok(OtxTypeRelation {
            input_type_in_base: self.type_in_input_range(otx.layout.base_inputs, type_hash),
            input_type_in_append: self.type_in_input_range(otx.layout.append_inputs, type_hash),
            output_type_in_base: self.type_in_output_range(otx.layout.base_outputs, type_hash),
            output_type_in_base_covered: self.covered_type_in_base_outputs(otx, type_hash)?,
            output_type_in_append: self.type_in_output_range(otx.layout.append_outputs, type_hash),
        })
    }

    pub(crate) fn type_hash_present(&self, type_hash: [u8; 32]) -> bool {
        self.input_types
            .iter()
            .chain(self.output_types.iter())
            .any(|hash| *hash == Some(type_hash))
    }

    pub(crate) fn type_hash_outside_otx_ranges(
        &self,
        type_hash: [u8; 32],
        otxs: &[OtxLayoutEntry],
    ) -> bool {
        self.input_types.iter().enumerate().any(|(index, hash)| {
            *hash == Some(type_hash)
                && !otxs.iter().any(|entry| {
                    range_contains(entry.layout.base_inputs, index)
                        || range_contains(entry.layout.append_inputs, index)
                })
        }) || self.output_types.iter().enumerate().any(|(index, hash)| {
            *hash == Some(type_hash)
                && !otxs.iter().any(|entry| {
                    range_contains(entry.layout.base_outputs, index)
                        || range_contains(entry.layout.append_outputs, index)
                })
        })
    }

    pub(crate) fn lock_hash_outside_otx_ranges(
        &self,
        lock_hash: [u8; 32],
        otxs: &[OtxLayoutEntry],
    ) -> bool {
        self.input_indices_for_lock(lock_hash).iter().any(|index| {
            !otxs.iter().any(|entry| {
                range_contains(entry.layout.base_inputs, *index)
                    || range_contains(entry.layout.append_inputs, *index)
            })
        })
    }

    pub(crate) fn all_inputs_with_lock_covered_by_otx(
        &self,
        lock_hash: [u8; 32],
        otxs: &[OtxLayoutEntry],
    ) -> bool {
        self.input_indices_for_lock(lock_hash).iter().all(|index| {
            otxs.iter().any(|entry| {
                range_contains(entry.layout.base_inputs, *index)
                    || range_contains(entry.layout.append_inputs, *index)
            })
        })
    }

    fn input_indices_for_lock(&self, lock_hash: [u8; 32]) -> &[usize] {
        self.lock_input_indices
            .iter()
            .find(|entry| entry.lock_hash == lock_hash)
            .map(|entry| entry.indices.as_slice())
            .unwrap_or(&[])
    }

    pub(crate) fn validate_message_targets(&self, message: &Cursor) -> Result<(), CoreError> {
        for action in MessageView::new(message.clone()).actions()? {
            let target_exists = match action.script_role {
                ScriptRole::InputLock => self.input_locks.contains(&action.script_hash),
                ScriptRole::InputType => self
                    .input_types
                    .iter()
                    .flatten()
                    .any(|hash| *hash == action.script_hash),
                ScriptRole::OutputType => self
                    .output_types
                    .iter()
                    .flatten()
                    .any(|hash| *hash == action.script_hash),
            };
            if !target_exists {
                return Err(CoreError::InvalidMessageTarget);
            }
        }
        Ok(())
    }

    fn covered_type_in_base_outputs(
        &self,
        otx: &OtxLayoutEntry,
        type_hash: [u8; 32],
    ) -> Result<bool, CoreError> {
        for local_index in 0..otx.layout.base_outputs.count {
            let tx_index = otx
                .layout
                .base_outputs
                .start
                .checked_add(local_index)
                .ok_or(CoreError::InvalidOtxLayout)?;
            if self.output_types.get(tx_index).copied().flatten() != Some(type_hash) {
                continue;
            }
            if otx.witness.base_output_masks.get(local_index * 4 + 2)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn range_contains(range: Range, index: usize) -> bool {
    index >= range.start && index < range.start.saturating_add(range.count)
}

fn index_input_locks(input_locks: &[[u8; 32]]) -> Vec<LockInputIndices> {
    let mut entries: Vec<LockInputIndices> = Vec::new();
    for (index, lock_hash) in input_locks.iter().copied().enumerate() {
        match entries
            .iter_mut()
            .find(|entry| entry.lock_hash == lock_hash)
        {
            Some(entry) => entry.indices.push(index),
            None => entries.push(LockInputIndices {
                lock_hash,
                indices: alloc::vec![index],
            }),
        }
    }
    entries
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
    fn range_queries_find_locks_and_types() {
        let lock_a = hash(1);
        let lock_b = hash(2);
        let type_a = hash(3);
        let hashes = TxScriptHashes::from_parts_for_tests(
            alloc::vec![lock_a, lock_b],
            alloc::vec![Some(type_a), None],
            alloc::vec![None, Some(type_a)],
        );

        assert!(hashes.type_hash_present(type_a));
    }

    #[test]
    fn lock_coverage_uses_cached_lock_input_indices() {
        let lock_a = hash(1);
        let lock_b = hash(2);
        let hashes = TxScriptHashes::from_parts_for_tests(
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

        assert!(hashes.all_inputs_with_lock_covered_by_otx(lock_a, &[layout_covering_lock_a]));
        assert!(!hashes.all_inputs_with_lock_covered_by_otx(lock_a, &[]));
    }

    #[test]
    fn validate_message_targets_accepts_existing_targets() {
        let lock_hash = hash(1);
        let input_type_hash = hash(2);
        let output_type_hash = hash(3);
        let hashes = TxScriptHashes::from_parts_for_tests(
            alloc::vec![lock_hash],
            alloc::vec![Some(input_type_hash)],
            alloc::vec![Some(output_type_hash)],
        );

        assert!(hashes
            .validate_message_targets(&message_with_action(0, lock_hash))
            .is_ok());
        assert!(hashes
            .validate_message_targets(&message_with_action(1, input_type_hash))
            .is_ok());
        assert!(hashes
            .validate_message_targets(&message_with_action(2, output_type_hash))
            .is_ok());
    }

    #[test]
    fn validate_message_targets_rejects_missing_or_unknown_targets() {
        let hashes = TxScriptHashes::default();

        for script_role in [0, 1, 2, 9] {
            assert_eq!(
                hashes.validate_message_targets(&message_with_action(script_role, hash(7))),
                Err(CoreError::InvalidMessageTarget)
            );
        }
    }
}
