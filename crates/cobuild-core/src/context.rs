use alloc::vec::Vec;

use crate::{
    error::CoreError,
    hash::{
        otx_append_hash, otx_base_hash, tx_with_message_hash, tx_without_message_hash, RawTxParts,
        TxHashParts,
    },
    layout::{build_layout, BuiltLayout, LayoutTx, Range},
    tasks::{OtxLockTask, OtxScope, TxLevelLockTask},
    view::{message_actions, TxLevelWitness, WitnessLayoutView},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TxScriptHashes {
    pub input_locks: Vec<[u8; 32]>,
    pub input_types: Vec<Option<[u8; 32]>>,
    pub output_types: Vec<Option<[u8; 32]>>,
}

pub struct CobuildContext {
    tx: LayoutTx,
    script_hashes: TxScriptHashes,
    layout: BuiltLayout,
    raw_parts: Option<RawTxParts>,
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

        let layout = build_layout(&tx)?;

        Ok(Self {
            tx,
            script_hashes,
            layout,
            raw_parts: None,
        })
    }

    pub fn with_raw_parts(
        tx: LayoutTx,
        script_hashes: TxScriptHashes,
        raw_parts: RawTxParts,
    ) -> Result<Self, CoreError> {
        let mut context = Self::new(tx, script_hashes)?;
        context.raw_parts = Some(raw_parts);
        Ok(context)
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
                self.validate_message_targets(message)?;
                let signing_message_hash = tx_with_message_hash(message, parts)?;
                (seal, signing_message_hash)
            }
            TxLevelWitness::SighashAllOnly { seal } => {
                let signing_message_hash = match tx_message {
                    Some(message) => {
                        self.validate_message_targets(&message)?;
                        tx_with_message_hash(&message, parts)?
                    }
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

    pub fn otx_tasks(&self, parts: &TxHashParts) -> Result<Vec<OtxLockTask>, CoreError> {
        let mut tasks = Vec::new();
        if self.context.layout.otx_data.is_empty() {
            return Ok(tasks);
        }
        let raw_parts = self
            .context
            .raw_parts
            .as_ref()
            .ok_or(CoreError::MissingHashParts)?;

        for otx in &self.context.layout.otx_data {
            let base_relevant = self.script_in_range(otx.layout.base_inputs);
            let append_relevant = self.script_in_range(otx.layout.append_inputs);
            if !base_relevant && !append_relevant {
                continue;
            }

            self.validate_message_targets(&otx.witness.message)?;
            let base_hash =
                otx_base_hash(&otx.witness, &otx.layout, raw_parts, &parts.resolved_inputs)?;
            if base_relevant {
                tasks.push(OtxLockTask {
                    script_hash: self.script_hash,
                    carrier_witness_index: otx.layout.witness_index,
                    scope: OtxScope::Base,
                    seal: self.unique_otx_seal(&otx.witness.seals, OtxScope::Base)?,
                    signing_message_hash: base_hash,
                });
            }
            if append_relevant {
                tasks.push(OtxLockTask {
                    script_hash: self.script_hash,
                    carrier_witness_index: otx.layout.witness_index,
                    scope: OtxScope::Append,
                    seal: self.unique_otx_seal(&otx.witness.seals, OtxScope::Append)?,
                    signing_message_hash: otx_append_hash(
                        &otx.witness,
                        &otx.layout,
                        raw_parts,
                        &parts.resolved_inputs,
                        base_hash,
                    )?,
                });
            }
        }

        Ok(tasks)
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

    fn script_in_range(&self, range: Range) -> bool {
        self.context
            .script_hashes
            .input_locks
            .iter()
            .skip(range.start)
            .take(range.count)
            .any(|hash| *hash == self.script_hash)
    }

    fn validate_message_targets(&self, message: &[u8]) -> Result<(), CoreError> {
        for action in message_actions(message)? {
            let target_exists = match action.script_role {
                0 => self
                    .context
                    .script_hashes
                    .input_locks
                    .iter()
                    .any(|hash| *hash == action.script_hash),
                1 => self
                    .context
                    .script_hashes
                    .input_types
                    .iter()
                    .flatten()
                    .any(|hash| *hash == action.script_hash),
                2 => self
                    .context
                    .script_hashes
                    .output_types
                    .iter()
                    .flatten()
                    .any(|hash| *hash == action.script_hash),
                _ => false,
            };
            if !target_exists {
                return Err(CoreError::InvalidMessageTarget);
            }
        }
        Ok(())
    }

    fn unique_otx_seal(
        &self,
        seals: &[crate::view::SealPairData],
        scope: OtxScope,
    ) -> Result<Vec<u8>, CoreError> {
        let scope = match scope {
            OtxScope::Base => 0,
            OtxScope::Append => 1,
        };
        let mut found = None;
        for seal in seals {
            if seal.scope > 1 {
                return Err(CoreError::InvalidLayout);
            }
            if seal.script_hash == self.script_hash && seal.scope == scope {
                if found.is_some() {
                    return Err(CoreError::DuplicateSealPair);
                }
                found = Some(seal.seal.clone());
            }
        }
        found.ok_or(CoreError::MissingSealPair)
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
