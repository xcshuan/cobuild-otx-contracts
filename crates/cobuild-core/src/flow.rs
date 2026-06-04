use crate::{context::ScriptHashIndex, layout::Range};
use alloc::vec::Vec;

use cobuild_types::lazy_reader::support::Cursor;

use crate::{error::CoreError, view::WitnessLayoutView};

pub(crate) fn first_input_with_lock(
    script_hashes: &ScriptHashIndex,
    lock_hash: [u8; 32],
) -> Option<usize> {
    script_hashes
        .input_locks
        .iter()
        .position(|hash| *hash == lock_hash)
}

pub(crate) fn unique_sighash_all_message(
    witnesses: &[Vec<u8>],
) -> Result<Option<Cursor>, CoreError> {
    let mut message = None;
    for witness in witnesses {
        if witness.is_empty() {
            continue;
        }
        let Ok(view) = WitnessLayoutView::from_slice(witness) else {
            continue;
        };
        if let Some(candidate) = view.sighash_all_message()? {
            if message.is_some() {
                return Err(CoreError::DuplicateSighashAll);
            }
            message = Some(candidate);
        }
    }
    Ok(message)
}

pub(crate) fn script_in_input_range(
    input_locks: &[[u8; 32]],
    range: Range,
    script_hash: [u8; 32],
) -> bool {
    input_locks
        .iter()
        .skip(range.start)
        .take(range.count)
        .any(|hash| *hash == script_hash)
}

pub(crate) fn type_hash_in_input_range(
    input_types: &[Option<[u8; 32]>],
    range: Range,
    type_hash: [u8; 32],
) -> bool {
    input_types
        .iter()
        .skip(range.start)
        .take(range.count)
        .any(|hash| *hash == Some(type_hash))
}

pub(crate) fn type_hash_in_output_range(
    output_types: &[Option<[u8; 32]>],
    range: Range,
    type_hash: [u8; 32],
) -> bool {
    output_types
        .iter()
        .skip(range.start)
        .take(range.count)
        .any(|hash| *hash == Some(type_hash))
}

pub(crate) fn covered_type_hash_in_base_outputs(
    output_types: &[Option<[u8; 32]>],
    range: Range,
    type_hash: [u8; 32],
    masks: &crate::view::MaskView,
) -> Result<bool, CoreError> {
    for local_index in 0..range.count {
        let tx_index = range
            .start
            .checked_add(local_index)
            .ok_or(CoreError::InvalidOtxLayout)?;
        if output_types.get(tx_index).copied().flatten() != Some(type_hash) {
            continue;
        }
        if masks.bit(local_index * 4 + 2)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn type_hash_present(
    input_types: &[Option<[u8; 32]>],
    output_types: &[Option<[u8; 32]>],
    type_hash: [u8; 32],
) -> bool {
    input_types
        .iter()
        .chain(output_types.iter())
        .any(|hash| *hash == Some(type_hash))
}

pub(crate) fn type_hash_outside_otx_ranges(
    input_types: &[Option<[u8; 32]>],
    output_types: &[Option<[u8; 32]>],
    type_hash: [u8; 32],
    otxs: &[crate::layout::OtxLayout],
) -> bool {
    input_types.iter().enumerate().any(|(index, hash)| {
        *hash == Some(type_hash)
            && !otxs.iter().any(|otx| {
                range_contains(otx.base_inputs, index) || range_contains(otx.append_inputs, index)
            })
    }) || output_types.iter().enumerate().any(|(index, hash)| {
        *hash == Some(type_hash)
            && !otxs.iter().any(|otx| {
                range_contains(otx.base_outputs, index) || range_contains(otx.append_outputs, index)
            })
    })
}

pub(crate) fn range_contains(range: Range, index: usize) -> bool {
    index >= range.start && index < range.start.saturating_add(range.count)
}

pub(crate) fn lock_group_fully_covered_by_otx(
    input_locks: &[[u8; 32]],
    lock_script_hash: [u8; 32],
    otxs: &[crate::layout::OtxLayout],
) -> bool {
    input_locks.iter().enumerate().all(|(index, hash)| {
        if *hash != lock_script_hash {
            return true;
        }
        otxs.iter().any(|otx| {
            range_contains(otx.base_inputs, index) || range_contains(otx.append_inputs, index)
        })
    })
}
