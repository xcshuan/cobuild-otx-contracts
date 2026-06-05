use alloc::vec::Vec;

use cobuild_types::lazy_reader::support::Cursor;

use crate::{
    error::CoreError,
    layout::{OtxLayout, OtxLayoutEntry, Range},
    plan::OtxTypeRelation,
    protocol::ScriptRole,
    syscalls::SyscallTxReader,
    view::MessageView,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TxScriptHashes {
    pub input_locks: Vec<[u8; 32]>,
    pub input_types: Vec<Option<[u8; 32]>>,
    pub output_types: Vec<Option<[u8; 32]>>,
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

        Ok(Self {
            input_locks,
            input_types,
            output_types,
        })
    }

    pub(crate) fn first_input_with_lock(&self, lock_hash: [u8; 32]) -> Option<usize> {
        self.input_locks.iter().position(|hash| *hash == lock_hash)
    }

    pub(crate) fn lock_in_input_range(&self, range: Range, lock_hash: [u8; 32]) -> bool {
        self.input_locks
            .iter()
            .skip(range.start)
            .take(range.count)
            .any(|hash| *hash == lock_hash)
    }

    pub(crate) fn type_in_input_range(&self, range: Range, type_hash: [u8; 32]) -> bool {
        self.input_types
            .iter()
            .skip(range.start)
            .take(range.count)
            .any(|hash| *hash == Some(type_hash))
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
        otxs: &[OtxLayout],
    ) -> bool {
        self.input_types.iter().enumerate().any(|(index, hash)| {
            *hash == Some(type_hash)
                && !otxs.iter().any(|otx| {
                    range_contains(otx.base_inputs, index)
                        || range_contains(otx.append_inputs, index)
                })
        }) || self.output_types.iter().enumerate().any(|(index, hash)| {
            *hash == Some(type_hash)
                && !otxs.iter().any(|otx| {
                    range_contains(otx.base_outputs, index)
                        || range_contains(otx.append_outputs, index)
                })
        })
    }

    pub(crate) fn lock_group_fully_covered_by_otx(
        &self,
        lock_hash: [u8; 32],
        otxs: &[OtxLayout],
    ) -> bool {
        self.input_locks.iter().enumerate().all(|(index, hash)| {
            if *hash != lock_hash {
                return true;
            }
            otxs.iter().any(|otx| {
                range_contains(otx.base_inputs, index) || range_contains(otx.append_inputs, index)
            })
        })
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
    fn range_queries_find_locks_and_types() {
        let lock_a = hash(1);
        let lock_b = hash(2);
        let type_a = hash(3);
        let hashes = TxScriptHashes {
            input_locks: alloc::vec![lock_a, lock_b],
            input_types: alloc::vec![Some(type_a), None],
            output_types: alloc::vec![None, Some(type_a)],
        };

        assert_eq!(hashes.first_input_with_lock(lock_a), Some(0));
        assert!(hashes.lock_in_input_range(range(0, 1), lock_a));
        assert!(!hashes.lock_in_input_range(range(1, 1), lock_a));
        assert!(hashes.type_hash_present(type_a));
    }

    #[test]
    fn lock_group_coverage_requires_every_matching_input_to_be_in_otx_ranges() {
        let lock_a = hash(1);
        let lock_b = hash(2);
        let hashes = TxScriptHashes {
            input_locks: alloc::vec![lock_a, lock_b, lock_a],
            input_types: alloc::vec![None, None, None],
            output_types: alloc::vec![],
        };
        let layout_covering_lock_a = OtxLayout {
            witness_index: 0,
            base_inputs: range(0, 1),
            append_inputs: range(2, 1),
            base_outputs: range(0, 0),
            append_outputs: range(0, 0),
            base_cell_deps: range(0, 0),
            append_cell_deps: range(0, 0),
            base_header_deps: range(0, 0),
            append_header_deps: range(0, 0),
        };

        assert!(hashes.lock_group_fully_covered_by_otx(lock_a, &[layout_covering_lock_a]));
        assert!(!hashes.lock_group_fully_covered_by_otx(lock_a, &[]));
    }

    #[test]
    fn validate_message_targets_accepts_existing_targets() {
        let lock_hash = hash(1);
        let input_type_hash = hash(2);
        let output_type_hash = hash(3);
        let hashes = TxScriptHashes {
            input_locks: alloc::vec![lock_hash],
            input_types: alloc::vec![Some(input_type_hash)],
            output_types: alloc::vec![Some(output_type_hash)],
        };

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
