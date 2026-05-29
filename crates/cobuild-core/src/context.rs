use alloc::vec::Vec;

use crate::{error::CoreError, layout::LayoutTx, tasks::TxLevelLockTask};

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
    pub fn tx_tasks(&self) -> Result<Vec<TxLevelLockTask>, CoreError> {
        if !self
            .context
            .script_hashes
            .input_locks
            .iter()
            .any(|hash| *hash == self.script_hash)
        {
            return Ok(Vec::new());
        }

        Ok(Vec::new())
    }
}
