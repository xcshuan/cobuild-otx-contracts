use alloc::boxed::Box;
use core::cmp::min;

use ckb_std::{ckb_constants::Source, error::SysError, high_level, syscalls};
use cobuild_types::lazy_reader::{
    blockchain::{RawTransaction, Transaction},
    support::{Cursor, Error as MoleculeError, Read},
};

use crate::error::CoreError;

#[derive(Clone, Copy)]
enum SyscallReadTarget {
    Transaction,
    ResolvedInputCell { index: usize },
    ResolvedInputData { index: usize },
}

impl SyscallReadTarget {
    fn load(&self, buf: &mut [u8], offset: usize) -> Result<usize, SysError> {
        match *self {
            Self::Transaction => syscalls::load_transaction(buf, offset),
            Self::ResolvedInputCell { index } => {
                syscalls::load_cell(buf, offset, index, Source::Input)
            }
            Self::ResolvedInputData { index } => {
                syscalls::load_cell_data(buf, offset, index, Source::Input)
            }
        }
    }
}

struct SyscallBackedReader {
    total_size: usize,
    target: SyscallReadTarget,
}

impl SyscallBackedReader {
    fn new(target: SyscallReadTarget) -> Result<Self, SysError> {
        let mut probe = [0u8; 1];
        let total_size = match target.load(&mut probe, 0) {
            Ok(size) => size,
            Err(SysError::LengthNotEnough(size)) => size,
            Err(err) => return Err(err),
        };
        Ok(Self { total_size, target })
    }
}

impl Read for SyscallBackedReader {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, MoleculeError> {
        if buf.is_empty() {
            return Ok(0);
        }
        if offset >= self.total_size {
            return Err(MoleculeError::OutOfBound(offset, self.total_size));
        }

        let read_len = min(buf.len(), self.total_size - offset);
        match self.target.load(&mut buf[..read_len], offset) {
            Ok(size) => Ok(min(size, read_len)),
            Err(err) => map_syscall_read_error(err, read_len),
        }
    }
}

fn map_syscall_read_error(err: SysError, read_len: usize) -> Result<usize, MoleculeError> {
    match err {
        SysError::LengthNotEnough(available) if available >= read_len => Ok(read_len),
        SysError::LengthNotEnough(available) => {
            Err(MoleculeError::Read(min(available, read_len), read_len))
        }
        _ => Err(MoleculeError::Read(0, read_len)),
    }
}

fn syscall_cursor(target: SyscallReadTarget, error: CoreError) -> Result<Cursor, CoreError> {
    let reader = SyscallBackedReader::new(target).map_err(|_| error)?;
    let total_size = reader.total_size;
    Ok(Cursor::new(total_size, Box::new(reader)))
}

fn hash_transaction_cursor() -> Result<Cursor, CoreError> {
    syscall_cursor(SyscallReadTarget::Transaction, CoreError::MissingHashInput)
}

fn resolved_input_output_cursor(index: usize) -> Result<Cursor, CoreError> {
    syscall_cursor(
        SyscallReadTarget::ResolvedInputCell { index },
        CoreError::MissingHashInput,
    )
}

fn resolved_input_data_cursor(index: usize) -> Result<Cursor, CoreError> {
    syscall_cursor(
        SyscallReadTarget::ResolvedInputData { index },
        CoreError::MissingHashInput,
    )
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TxCounts {
    pub inputs: usize,
    pub outputs: usize,
    pub cell_deps: usize,
    pub header_deps: usize,
    pub witnesses: usize,
}

#[derive(Default)]
pub(crate) struct SyscallTxReader {
    counts: TxCounts,
}

impl SyscallTxReader {
    pub(super) fn preload_counts_from_syscalls(&mut self) -> Result<(), CoreError> {
        self.counts = read_counts_from_transaction()?;
        Ok(())
    }

    pub(super) fn counts(&self) -> TxCounts {
        self.counts
    }

    pub(super) fn tx_hash(&self) -> Result<[u8; 32], CoreError> {
        tx_hash()
    }

    pub(super) fn witness_cursor(&self, absolute_index: usize) -> Result<Cursor, CoreError> {
        witness_cursor(absolute_index)
    }

    pub(super) fn raw_input_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        raw_input_cursor(index)
    }

    pub(super) fn raw_output_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        raw_output_cursor(index)
    }

    pub(super) fn raw_output_data_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        raw_output_data_cursor(index)
    }

    pub(super) fn raw_cell_dep_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        raw_cell_dep_cursor(index)
    }

    pub(super) fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        raw_header_dep_hash(index)
    }

    pub(super) fn resolved_input_output_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        resolved_input_output_cursor(index)
    }

    pub(super) fn resolved_input_data_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        resolved_input_data_cursor(index)
    }

    pub(super) fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        input_lock_hash(index)
    }

    pub(super) fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        input_type_hash(index)
    }

    pub(super) fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        output_type_hash(index)
    }
}

fn transaction_view_for_hash() -> Result<Transaction, CoreError> {
    hash_transaction_cursor().map(Transaction::from)
}

fn raw_transaction_for_hash() -> Result<RawTransaction, CoreError> {
    transaction_view_for_hash()?
        .raw()
        .map_err(|_| CoreError::MissingHashInput)
}

fn read_counts_from_transaction() -> Result<TxCounts, CoreError> {
    let tx = transaction_view_for_hash()?;
    let raw = tx.raw().map_err(|_| CoreError::MissingHashInput)?;
    Ok(TxCounts {
        inputs: raw
            .inputs()
            .and_then(|inputs| inputs.len())
            .map_err(|_| CoreError::MissingHashInput)?,
        outputs: raw
            .outputs()
            .and_then(|outputs| outputs.len())
            .map_err(|_| CoreError::MissingHashInput)?,
        cell_deps: raw
            .cell_deps()
            .and_then(|cell_deps| cell_deps.len())
            .map_err(|_| CoreError::MissingHashInput)?,
        header_deps: raw
            .header_deps()
            .and_then(|header_deps| header_deps.len())
            .map_err(|_| CoreError::MissingHashInput)?,
        witnesses: tx
            .witnesses()
            .and_then(|witnesses| witnesses.len())
            .map_err(|_| CoreError::MissingHashInput)?,
    })
}

fn witness_cursor(absolute_index: usize) -> Result<Cursor, CoreError> {
    transaction_view_for_hash()?
        .witnesses()
        .and_then(|witnesses| witnesses.get(absolute_index))
        .map_err(|_| CoreError::MissingHashInput)
}

fn raw_input_cursor(index: usize) -> Result<Cursor, CoreError> {
    Ok(raw_transaction_for_hash()?
        .inputs()
        .and_then(|inputs| inputs.get(index))
        .map_err(|_| CoreError::MissingHashInput)?
        .cursor)
}

fn raw_output_cursor(index: usize) -> Result<Cursor, CoreError> {
    Ok(raw_transaction_for_hash()?
        .outputs()
        .and_then(|outputs| outputs.get(index))
        .map_err(|_| CoreError::MissingHashInput)?
        .cursor)
}

fn raw_output_data_cursor(index: usize) -> Result<Cursor, CoreError> {
    raw_transaction_for_hash()?
        .outputs_data()
        .and_then(|outputs_data| outputs_data.get(index))
        .map_err(|_| CoreError::MissingHashInput)
}

fn raw_cell_dep_cursor(index: usize) -> Result<Cursor, CoreError> {
    Ok(raw_transaction_for_hash()?
        .cell_deps()
        .and_then(|cell_deps| cell_deps.get(index))
        .map_err(|_| CoreError::MissingHashInput)?
        .cursor)
}

fn raw_header_dep_hash(index: usize) -> Result<[u8; 32], CoreError> {
    raw_transaction_for_hash()?
        .header_deps()
        .and_then(|header_deps| header_deps.get(index))
        .map_err(|_| CoreError::MissingHashInput)
}

fn tx_hash() -> Result<[u8; 32], CoreError> {
    high_level::load_tx_hash().map_err(|_| CoreError::InvalidContextInput)
}

fn input_lock_hash(index: usize) -> Result<[u8; 32], CoreError> {
    high_level::load_cell_lock_hash(index, Source::Input)
        .map_err(|_| CoreError::InvalidContextInput)
}

fn input_type_hash(index: usize) -> Result<Option<[u8; 32]>, CoreError> {
    high_level::load_cell_type_hash(index, Source::Input)
        .map_err(|_| CoreError::InvalidContextInput)
}

fn output_type_hash(index: usize) -> Result<Option<[u8; 32]>, CoreError> {
    high_level::load_cell_type_hash(index, Source::Output)
        .map_err(|_| CoreError::InvalidContextInput)
}

#[cfg(test)]
mod tests {
    #[test]
    fn reader_returns_explicit_counts() {
        let counts = super::TxCounts {
            inputs: 1,
            outputs: 2,
            cell_deps: 3,
            header_deps: 4,
            witnesses: 5,
        };
        let reader = super::SyscallTxReader { counts };

        assert_eq!(reader.counts(), counts);
    }
}
