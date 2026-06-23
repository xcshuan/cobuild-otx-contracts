use alloc::vec::Vec;

use crate::{error::CoreError, reader::cursor_bytes, view::LockSealView};

pub(crate) fn unique_lock_seal(
    script_hash: [u8; 32],
    seals: &[LockSealView],
) -> Result<Vec<u8>, CoreError> {
    let mut found = None;
    for seal in seals {
        if seal.script_hash == script_hash {
            if found.is_some() {
                return Err(CoreError::DuplicateLockSeal);
            }
            found = Some(cursor_bytes(&seal.seal)?);
        }
    }
    found.ok_or(CoreError::MissingLockSeal)
}
