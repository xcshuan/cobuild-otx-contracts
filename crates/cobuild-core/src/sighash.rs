use alloc::vec::Vec;

use cobuild_types::lazy_reader::support::Cursor;

use crate::{
    context::LockScriptQuery,
    error::CoreError,
    hash::{tx_with_message_hash, tx_without_message_hash},
    reader::cursor_bytes,
    signature::{SignatureOrigin, SignatureRequest},
    source::HashInputSource,
    view::{SighashAllWitnessView, WitnessLayoutView},
};

impl LockScriptQuery<'_> {
    pub(crate) fn collect_sighash_all_signatures<S: HashInputSource>(
        &self,
        source: &S,
    ) -> Result<Vec<SignatureRequest>, CoreError> {
        let Some(carrier_witness_index) = self
            .context
            .script_hashes
            .input_locks
            .iter()
            .position(|hash| *hash == self.script_hash)
        else {
            return Ok(Vec::new());
        };

        let Some(witness) = self.context.tx.witnesses.get(carrier_witness_index) else {
            return Ok(Vec::new());
        };
        if witness.is_empty() {
            return Ok(Vec::new());
        }

        let view = WitnessLayoutView::from_slice(witness)?;
        let Some(sighash_all_witness_layout) = view.sighash_all_witness_layout()? else {
            return Ok(Vec::new());
        };
        let tx_message = self.unique_sighash_all_message()?;
        let (seal, signing_message_hash) = match sighash_all_witness_layout {
            SighashAllWitnessView::WithMessage { seal, message } => {
                let message = tx_message.as_ref().unwrap_or(&message);
                self.validate_message_targets(message)?;
                let signing_message_hash = tx_with_message_hash(message, source)?;
                (cursor_bytes(&seal)?, signing_message_hash)
            }
            SighashAllWitnessView::SealOnly { seal } => {
                let signing_message_hash = match tx_message {
                    Some(message) => {
                        self.validate_message_targets(&message)?;
                        tx_with_message_hash(&message, source)?
                    }
                    None => tx_without_message_hash(source)?,
                };
                (cursor_bytes(&seal)?, signing_message_hash)
            }
        };

        Ok(alloc::vec![SignatureRequest {
            script_hash: self.script_hash,
            carrier_witness_index,
            origin: SignatureOrigin::SighashAll,
            seal,
            signing_message_hash,
        }])
    }

    pub(crate) fn unique_sighash_all_message(&self) -> Result<Option<Cursor>, CoreError> {
        let mut message = None;
        for witness in &self.context.tx.witnesses {
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
}
