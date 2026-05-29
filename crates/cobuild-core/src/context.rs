use alloc::vec::Vec;

use crate::{
    error::CoreError,
    hash::{tx_with_message_hash, tx_without_message_hash, TxHashParts},
    layout::LayoutTx,
    tasks::TxLevelLockTask,
    view::{TxLevelWitness, WitnessLayoutView},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TxScriptHashes {
    pub input_locks: Vec<[u8; 32]>,
    pub input_types: Vec<Option<[u8; 32]>>,
    pub output_types: Vec<Option<[u8; 32]>>,
}

pub struct CobuildContext {
    #[allow(dead_code)]
    tx: LayoutTx,
    script_hashes: TxScriptHashes,
}

pub struct LockScriptQuery<'a> {
    context: &'a CobuildContext,
    script_hash: [u8; 32],
}

impl CobuildContext {
    pub fn new(tx: LayoutTx, script_hashes: TxScriptHashes) -> Result<Self, CoreError> {
        if script_hashes.input_locks.len() != tx.input_count {
            return Err(CoreError::InvalidLayout);
        }
        if script_hashes.input_types.len() != tx.input_count {
            return Err(CoreError::InvalidLayout);
        }
        if script_hashes.output_types.len() != tx.output_count {
            return Err(CoreError::InvalidLayout);
        }

        Ok(Self { tx, script_hashes })
    }

    pub fn lock_query(&self, script_hash: [u8; 32]) -> LockScriptQuery<'_> {
        LockScriptQuery {
            context: self,
            script_hash,
        }
    }
}

impl LockScriptQuery<'_> {
    pub fn tx_tasks(&self, parts: &TxHashParts) -> Result<Vec<TxLevelLockTask>, CoreError> {
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
        let Some(tx_level_witness) = view.tx_level_witness()? else {
            return Ok(Vec::new());
        };
        let tx_message = self.unique_sighash_all_message()?;
        let (seal, signing_message_hash) = match tx_level_witness {
            TxLevelWitness::SighashAll { seal, message } => {
                let message = tx_message.as_deref().unwrap_or(&message);
                let signing_message_hash = tx_with_message_hash(message, parts)?;
                (seal, signing_message_hash)
            }
            TxLevelWitness::SighashAllOnly { seal } => {
                let signing_message_hash = match tx_message {
                    Some(message) => tx_with_message_hash(&message, parts)?,
                    None => tx_without_message_hash(parts)?,
                };
                (seal, signing_message_hash)
            }
        };

        Ok(alloc::vec![TxLevelLockTask {
            script_hash: self.script_hash,
            carrier_witness_index,
            seal,
            signing_message_hash,
        }])
    }

    fn unique_sighash_all_message(&self) -> Result<Option<Vec<u8>>, CoreError> {
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
                    return Err(CoreError::DuplicateSealPair);
                }
                message = Some(candidate);
            }
        }
        Ok(message)
    }
}

pub struct PreparedContext {
    pub context: CobuildContext,
    pub hash_parts: TxHashParts,
}

impl PreparedContext {
    pub fn new(context: CobuildContext, hash_parts: TxHashParts) -> Self {
        Self {
            context,
            hash_parts,
        }
    }
}
