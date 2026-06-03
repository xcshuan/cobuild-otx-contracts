use alloc::vec::Vec;

use crate::{
    error::CoreError,
    layout::{scan_layout, LayoutTx, OtxLayoutScan},
    source::InMemorySource,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TxScriptHashes {
    pub input_locks: Vec<[u8; 32]>,
    pub input_types: Vec<Option<[u8; 32]>>,
    pub output_types: Vec<Option<[u8; 32]>>,
}

pub struct CobuildContext {
    pub(crate) tx: LayoutTx,
    pub(crate) script_hashes: TxScriptHashes,
    pub(crate) layout_scan: OtxLayoutScan,
}

pub struct LockScriptQuery<'a> {
    pub(crate) context: &'a CobuildContext,
    pub(crate) script_hash: [u8; 32],
}

impl CobuildContext {
    pub fn new(tx: LayoutTx, script_hashes: TxScriptHashes) -> Result<Self, CoreError> {
        if script_hashes.input_locks.len() != tx.input_count {
            return Err(CoreError::InvalidContextInput);
        }
        if script_hashes.input_types.len() != tx.input_count {
            return Err(CoreError::InvalidContextInput);
        }
        if script_hashes.output_types.len() != tx.output_count {
            return Err(CoreError::InvalidContextInput);
        }

        let layout_scan = scan_layout(&tx);

        Ok(Self {
            tx,
            script_hashes,
            layout_scan,
        })
    }

    pub fn lock_query(&self, script_hash: [u8; 32]) -> LockScriptQuery<'_> {
        LockScriptQuery {
            context: self,
            script_hash,
        }
    }
}

pub struct PreparedContext {
    pub context: CobuildContext,
    pub signing_source: InMemorySource,
}

impl PreparedContext {
    pub fn new(context: CobuildContext, signing_source: InMemorySource) -> Self {
        Self {
            context,
            signing_source,
        }
    }
}
