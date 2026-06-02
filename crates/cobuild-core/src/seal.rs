use alloc::vec::Vec;

use crate::{context::LockScriptQuery, error::CoreError, protocol::SealScope, view::SealPairData};

impl LockScriptQuery<'_> {
    pub(crate) fn unique_otx_base_seal(
        &self,
        seals: &[SealPairData],
    ) -> Result<Vec<u8>, CoreError> {
        self.unique_otx_seal_by_scope(seals, SealScope::Base)
    }

    pub(crate) fn unique_otx_append_seal(
        &self,
        seals: &[SealPairData],
    ) -> Result<Vec<u8>, CoreError> {
        self.unique_otx_seal_by_scope(seals, SealScope::Append)
    }

    fn unique_otx_seal_by_scope(
        &self,
        seals: &[SealPairData],
        scope: SealScope,
    ) -> Result<Vec<u8>, CoreError> {
        let mut found = None;
        for seal in seals {
            let seal_scope = SealScope::try_from(seal.scope)?;
            if seal.script_hash == self.script_hash && seal_scope == scope {
                if found.is_some() {
                    return Err(CoreError::DuplicateSealPair);
                }
                found = Some(seal.seal.clone());
            }
        }
        found.ok_or(CoreError::MissingSealPair)
    }
}
