use alloc::vec::Vec;

use crate::{error::CoreError, protocol::SealScope, reader::cursor_bytes, view::SealPairView};

pub(crate) fn unique_otx_seal_by_scope(
    script_hash: [u8; 32],
    seals: &[SealPairView],
    scope: SealScope,
) -> Result<Vec<u8>, CoreError> {
    let mut found = None;
    for seal in seals {
        let seal_scope = SealScope::try_from(seal.scope)?;
        if seal.script_hash == script_hash && seal_scope == scope {
            if found.is_some() {
                return Err(CoreError::DuplicateSealPair);
            }
            found = Some(cursor_bytes(&seal.seal)?);
        }
    }
    found.ok_or(CoreError::MissingSealPair)
}
