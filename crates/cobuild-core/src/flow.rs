use crate::context::ScriptHashIndex;
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
