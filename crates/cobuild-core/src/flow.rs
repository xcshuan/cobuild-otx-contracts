use crate::context::ScriptHashIndex;

pub(crate) fn first_input_with_lock(
    script_hashes: &ScriptHashIndex,
    lock_hash: [u8; 32],
) -> Option<usize> {
    script_hashes
        .input_locks
        .iter()
        .position(|hash| *hash == lock_hash)
}
