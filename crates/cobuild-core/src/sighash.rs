use alloc::vec::Vec;

use crate::{
    context::LockScriptQuery,
    error::CoreError,
    hash::{tx_with_message_hash, tx_without_message_hash},
    reader::cursor_from_slice,
    signature::{SignatureOrigin, SignatureRequest},
    source::SigningDataSource,
    view::{SighashAllWitnessLayout, WitnessLayoutView},
};

impl LockScriptQuery<'_> {
    pub(crate) fn collect_sighash_all_signatures<S: SigningDataSource>(
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
            SighashAllWitnessLayout::WithMessage { seal, message } => {
                let message = tx_message.as_deref().unwrap_or(&message);
                self.validate_message_targets(message)?;
                let message = cursor_from_slice(message);
                let signing_message_hash = tx_with_message_hash(&message, source)?;
                (seal, signing_message_hash)
            }
            SighashAllWitnessLayout::SealOnly { seal } => {
                let signing_message_hash = match tx_message {
                    Some(message) => {
                        self.validate_message_targets(&message)?;
                        let message = cursor_from_slice(&message);
                        tx_with_message_hash(&message, source)?
                    }
                    None => tx_without_message_hash(source)?,
                };
                (seal, signing_message_hash)
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

    pub(crate) fn unique_sighash_all_message(&self) -> Result<Option<Vec<u8>>, CoreError> {
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
