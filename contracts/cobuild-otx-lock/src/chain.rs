mod reader;

use ckb_std::{ckb_constants::Source, error::SysError, high_level};
use cobuild_core::{
    engine::{CobuildEngine, PreparedCobuild},
    error::CoreError,
    source::{ClassifiedCursor, HashInputSource, TransactionSource, TxCounts},
};
use cobuild_types::lazy_reader::{
    blockchain::{RawTransaction, Transaction},
    support::Cursor,
};

use self::reader::{
    resolved_input_cell_cursor, resolved_input_data_cursor, script_cursor, transaction_cursor,
};
use crate::error::Error;

pub(crate) struct PreparedCobuildContext {
    pub tx_reader: SyscallTxReader,
    pub prepared: PreparedCobuild,
}

pub(crate) fn prepare_cobuild_from_syscalls() -> Result<PreparedCobuildContext, Error> {
    let tx_reader = SyscallTxReader::default();
    let prepared = CobuildEngine::prepare(&tx_reader)?;
    Ok(PreparedCobuildContext {
        tx_reader,
        prepared,
    })
}

#[derive(Default)]
struct TxCountsCache {
    counts: core::cell::Cell<Option<TxCounts>>,
}

impl TxCountsCache {
    fn counts(&self) -> Option<TxCounts> {
        self.counts.get()
    }

    fn set_counts(&self, counts: TxCounts) {
        self.counts.set(Some(counts));
    }
}

#[derive(Default)]
pub(crate) struct SyscallTxReader {
    cache: TxCountsCache,
}

fn context_cursor(cursor: Result<Cursor, SysError>) -> Result<ClassifiedCursor, CoreError> {
    cursor
        .map(ClassifiedCursor::source_input)
        .map_err(|_| CoreError::InvalidContextInput)
}

fn hash_cursor(cursor: Result<Cursor, SysError>) -> Result<ClassifiedCursor, CoreError> {
    cursor
        .map(ClassifiedCursor::hash_input)
        .map_err(|_| CoreError::MissingHashInput)
}

fn signing_transaction_view() -> Result<Transaction, CoreError> {
    transaction_cursor()
        .map(Transaction::from)
        .map_err(|_| CoreError::MissingHashInput)
}

fn signing_raw_transaction() -> Result<RawTransaction, CoreError> {
    signing_transaction_view()?
        .raw()
        .map_err(|_| CoreError::MissingHashInput)
}

impl TransactionSource for SyscallTxReader {
    fn transaction_cursor(&self) -> Result<ClassifiedCursor, CoreError> {
        context_cursor(transaction_cursor())
    }

    fn script_cursor(&self) -> Result<ClassifiedCursor, CoreError> {
        context_cursor(script_cursor())
    }

    fn tx_hash(&self) -> Result<[u8; 32], CoreError> {
        high_level::load_tx_hash().map_err(|_| CoreError::InvalidContextInput)
    }

    fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        high_level::load_cell_lock_hash(index, Source::Input)
            .map_err(|_| CoreError::InvalidContextInput)
    }

    fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        high_level::load_cell_type_hash(index, Source::Input)
            .map_err(|_| CoreError::InvalidContextInput)
    }

    fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        high_level::load_cell_type_hash(index, Source::Output)
            .map_err(|_| CoreError::InvalidContextInput)
    }
}

impl HashInputSource for SyscallTxReader {
    fn counts(&self) -> Result<TxCounts, CoreError> {
        if let Some(counts) = self.cache.counts() {
            return Ok(counts);
        }

        let tx = signing_transaction_view()?;
        let raw = tx.raw().map_err(|_| CoreError::MissingHashInput)?;
        let inputs = raw
            .inputs()
            .and_then(|inputs| inputs.len())
            .map_err(|_| CoreError::MissingHashInput)?;
        let outputs = raw
            .outputs()
            .and_then(|outputs| outputs.len())
            .map_err(|_| CoreError::MissingHashInput)?;
        let cell_deps = raw
            .cell_deps()
            .and_then(|cell_deps| cell_deps.len())
            .map_err(|_| CoreError::MissingHashInput)?;
        let header_deps = raw
            .header_deps()
            .and_then(|header_deps| header_deps.len())
            .map_err(|_| CoreError::MissingHashInput)?;
        let witnesses = tx
            .witnesses()
            .and_then(|witnesses| witnesses.len())
            .map_err(|_| CoreError::MissingHashInput)?;
        let counts = TxCounts {
            inputs,
            outputs,
            cell_deps,
            header_deps,
            witnesses,
        };
        self.cache.set_counts(counts);
        Ok(counts)
    }

    fn witness_cursor(&self, absolute_index: usize) -> Result<ClassifiedCursor, CoreError> {
        let cursor = signing_transaction_view()?
            .witnesses()
            .and_then(|witnesses| witnesses.get(absolute_index))
            .map_err(|_| CoreError::MissingHashInput)?;
        Ok(ClassifiedCursor::hash_input(cursor))
    }

    fn raw_input_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        let input = signing_raw_transaction()?
            .inputs()
            .and_then(|inputs| inputs.get(index))
            .map_err(|_| CoreError::MissingHashInput)?;
        Ok(ClassifiedCursor::hash_input(input.cursor))
    }

    fn raw_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        let output = signing_raw_transaction()?
            .outputs()
            .and_then(|outputs| outputs.get(index))
            .map_err(|_| CoreError::MissingHashInput)?;
        Ok(ClassifiedCursor::hash_input(output.cursor))
    }

    fn raw_output_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        let cursor = signing_raw_transaction()?
            .outputs_data()
            .and_then(|outputs_data| outputs_data.get(index))
            .map_err(|_| CoreError::MissingHashInput)?;
        Ok(ClassifiedCursor::hash_input(cursor))
    }

    fn raw_cell_dep_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        let cell_dep = signing_raw_transaction()?
            .cell_deps()
            .and_then(|cell_deps| cell_deps.get(index))
            .map_err(|_| CoreError::MissingHashInput)?;
        Ok(ClassifiedCursor::hash_input(cell_dep.cursor))
    }

    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        signing_raw_transaction()?
            .header_deps()
            .and_then(|header_deps| header_deps.get(index))
            .map_err(|_| CoreError::MissingHashInput)
    }

    fn resolved_input_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_cursor(resolved_input_cell_cursor(index))
    }

    fn resolved_input_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_cursor(resolved_input_data_cursor(index))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn cached_counts_are_returned_without_recomputing() {
        let counts = cobuild_core::source::TxCounts {
            inputs: 1,
            outputs: 2,
            cell_deps: 3,
            header_deps: 4,
            witnesses: 5,
        };
        let cache = super::TxCountsCache::default();

        cache.set_counts(counts);

        assert_eq!(cache.counts(), Some(counts));
    }
}
