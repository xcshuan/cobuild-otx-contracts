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
