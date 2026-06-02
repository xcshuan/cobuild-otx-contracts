use alloc::vec::Vec;

use crate::{
    context::LockScriptQuery, error::CoreError, hash::SigningHashParts, signature::SignatureRequest,
};

impl LockScriptQuery<'_> {
    pub fn required_signatures(
        &self,
        parts: &SigningHashParts,
    ) -> Result<Vec<SignatureRequest>, CoreError> {
        let sighash_all_requests = self.collect_sighash_all_signatures(parts)?;
        let otx_requests = self.collect_otx_signatures(parts)?;
        if !otx_requests.is_empty() && sighash_all_requests.is_empty() {
            self.ensure_otx_covers_current_lock_group()?;
        }

        let mut requests = sighash_all_requests;
        requests.extend(otx_requests);
        Ok(requests)
    }

    fn ensure_otx_covers_current_lock_group(&self) -> Result<(), CoreError> {
        for (index, hash) in self.context.script_hashes.input_locks.iter().enumerate() {
            if *hash == self.script_hash && !self.otx_covers_input(index) {
                return Err(CoreError::MissingLockGroupCoverage);
            }
        }
        Ok(())
    }
}
