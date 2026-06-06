use alloc::boxed::Box;
#[cfg(test)]
use alloc::vec::Vec;
use core::cmp::min;

use ckb_std::{ckb_constants::Source, error::SysError, high_level, syscalls};
use cobuild_types::lazy_reader::{
    blockchain::{RawTransaction, Transaction},
    support::{Cursor, Error as MoleculeError, Read},
};

use crate::error::CoreError;

struct TransactionReader {
    total_size: usize,
}

impl TransactionReader {
    fn new() -> Result<Self, SysError> {
        let total_size = read_syscall_size(|buf| syscalls::load_transaction(buf, 0))?;
        Ok(Self { total_size })
    }
}

impl Read for TransactionReader {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, MoleculeError> {
        read_syscall_data(self.total_size, buf, offset, syscalls::load_transaction)
    }
}

impl From<TransactionReader> for Cursor {
    fn from(reader: TransactionReader) -> Self {
        Cursor::new(reader.total_size, Box::new(reader))
    }
}

struct ResolvedInputCellReader {
    total_size: usize,
    index: usize,
}

impl ResolvedInputCellReader {
    fn new(index: usize) -> Result<Self, SysError> {
        let total_size =
            read_syscall_size(|buf| syscalls::load_cell(buf, 0, index, Source::Input))?;
        Ok(Self { total_size, index })
    }
}

impl Read for ResolvedInputCellReader {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, MoleculeError> {
        read_syscall_data(self.total_size, buf, offset, |buf, offset| {
            syscalls::load_cell(buf, offset, self.index, Source::Input)
        })
    }
}

impl From<ResolvedInputCellReader> for Cursor {
    fn from(reader: ResolvedInputCellReader) -> Self {
        Cursor::new(reader.total_size, Box::new(reader))
    }
}

struct ResolvedInputDataReader {
    total_size: usize,
    index: usize,
}

impl ResolvedInputDataReader {
    fn new(index: usize) -> Result<Self, SysError> {
        let total_size =
            read_syscall_size(|buf| syscalls::load_cell_data(buf, 0, index, Source::Input))?;
        Ok(Self { total_size, index })
    }
}

impl Read for ResolvedInputDataReader {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, MoleculeError> {
        read_syscall_data(self.total_size, buf, offset, |buf, offset| {
            syscalls::load_cell_data(buf, offset, self.index, Source::Input)
        })
    }
}

impl From<ResolvedInputDataReader> for Cursor {
    fn from(reader: ResolvedInputDataReader) -> Self {
        Cursor::new(reader.total_size, Box::new(reader))
    }
}

fn read_syscall_size<F>(load: F) -> Result<usize, SysError>
where
    F: FnOnce(&mut [u8]) -> Result<usize, SysError>,
{
    let mut probe = [0u8; 1];
    match load(&mut probe) {
        Ok(size) => Ok(size),
        Err(SysError::LengthNotEnough(size)) => Ok(size),
        Err(err) => Err(err),
    }
}

fn read_syscall_data<F>(
    total_size: usize,
    buf: &mut [u8],
    offset: usize,
    load: F,
) -> Result<usize, MoleculeError>
where
    F: FnOnce(&mut [u8], usize) -> Result<usize, SysError>,
{
    if buf.is_empty() {
        return Ok(0);
    }
    if offset >= total_size {
        return Err(MoleculeError::OutOfBound(offset, total_size));
    }

    let read_len = min(buf.len(), total_size - offset);
    match load(&mut buf[..read_len], offset) {
        Ok(size) => Ok(min(size, read_len)),
        Err(err) => map_syscall_read_error(err, read_len),
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

fn transaction_cursor_from_syscalls() -> Result<Cursor, CoreError> {
    let reader = TransactionReader::new().map_err(|_| CoreError::MissingHashInput)?;
    Ok(reader.into())
}

fn resolved_input_output_cursor(index: usize) -> Result<Cursor, CoreError> {
    let reader = ResolvedInputCellReader::new(index).map_err(|_| CoreError::MissingHashInput)?;
    Ok(reader.into())
}

fn resolved_input_data_cursor(index: usize) -> Result<Cursor, CoreError> {
    let reader = ResolvedInputDataReader::new(index).map_err(|_| CoreError::MissingHashInput)?;
    Ok(reader.into())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TxCounts {
    pub inputs: usize,
    pub outputs: usize,
    pub cell_deps: usize,
    pub header_deps: usize,
    pub witnesses: usize,
}

pub(crate) struct SyscallTxReader {
    counts: TxCounts,
    transaction: Cursor,
    tx_hash: [u8; 32],
    #[cfg(test)]
    cell_script_hashes: CellScriptHashesForTests,
}

impl SyscallTxReader {
    pub(super) fn from_syscalls() -> Result<Self, CoreError> {
        let transaction = transaction_cursor_from_syscalls()?;
        let counts = read_counts_from_transaction(&transaction)?;
        let tx_hash = tx_hash_from_syscalls()?;
        Ok(Self {
            counts,
            transaction,
            tx_hash,
            #[cfg(test)]
            cell_script_hashes: CellScriptHashesForTests::default(),
        })
    }

    #[cfg(test)]
    pub(crate) fn from_cached_parts_for_tests(
        counts: TxCounts,
        transaction: Cursor,
        tx_hash: [u8; 32],
    ) -> Self {
        Self {
            counts,
            transaction,
            tx_hash,
            cell_script_hashes: CellScriptHashesForTests::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_cell_script_hashes_for_tests(
        mut self,
        input_locks: Vec<[u8; 32]>,
        input_types: Vec<Option<[u8; 32]>>,
        output_types: Vec<Option<[u8; 32]>>,
    ) -> Self {
        assert_eq!(input_locks.len(), input_types.len());
        self.counts.inputs = input_locks.len();
        self.counts.outputs = output_types.len();
        self.cell_script_hashes = CellScriptHashesForTests {
            input_locks,
            input_types,
            output_types,
        };
        self
    }

    pub(super) fn counts(&self) -> TxCounts {
        self.counts
    }

    pub(super) fn tx_hash(&self) -> [u8; 32] {
        self.tx_hash
    }

    #[cfg(test)]
    fn transaction_cursor(&self) -> Cursor {
        self.transaction.clone()
    }

    pub(super) fn witness_cursor(&self, absolute_index: usize) -> Result<Cursor, CoreError> {
        self.transaction_view()
            .witnesses()
            .and_then(|witnesses| witnesses.get(absolute_index))
            .map_err(|_| CoreError::MissingHashInput)
    }

    pub(super) fn raw_input_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        Ok(self
            .raw_transaction_view()?
            .inputs()
            .and_then(|inputs| inputs.get(index))
            .map_err(|_| CoreError::MissingHashInput)?
            .cursor)
    }

    pub(super) fn raw_output_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        Ok(self
            .raw_transaction_view()?
            .outputs()
            .and_then(|outputs| outputs.get(index))
            .map_err(|_| CoreError::MissingHashInput)?
            .cursor)
    }

    pub(super) fn raw_output_data_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        self.raw_transaction_view()?
            .outputs_data()
            .and_then(|outputs_data| outputs_data.get(index))
            .map_err(|_| CoreError::MissingHashInput)
    }

    pub(super) fn raw_cell_dep_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        Ok(self
            .raw_transaction_view()?
            .cell_deps()
            .and_then(|cell_deps| cell_deps.get(index))
            .map_err(|_| CoreError::MissingHashInput)?
            .cursor)
    }

    pub(super) fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        self.raw_transaction_view()?
            .header_deps()
            .and_then(|header_deps| header_deps.get(index))
            .map_err(|_| CoreError::MissingHashInput)
    }

    pub(super) fn resolved_input_output_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        resolved_input_output_cursor(index)
    }

    pub(super) fn resolved_input_data_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        resolved_input_data_cursor(index)
    }

    pub(super) fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        #[cfg(test)]
        if let Some(hash) = self.cell_script_hashes.input_locks.get(index).copied() {
            return Ok(hash);
        }
        input_lock_hash(index)
    }

    pub(super) fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        #[cfg(test)]
        if let Some(hash) = self.cell_script_hashes.input_types.get(index).copied() {
            return Ok(hash);
        }
        input_type_hash(index)
    }

    pub(super) fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        #[cfg(test)]
        if let Some(hash) = self.cell_script_hashes.output_types.get(index).copied() {
            return Ok(hash);
        }
        output_type_hash(index)
    }

    fn transaction_view(&self) -> Transaction {
        Transaction::from(self.transaction.clone())
    }

    fn raw_transaction_view(&self) -> Result<RawTransaction, CoreError> {
        self.transaction_view()
            .raw()
            .map_err(|_| CoreError::MissingHashInput)
    }
}

#[cfg(test)]
#[derive(Default)]
struct CellScriptHashesForTests {
    input_locks: Vec<[u8; 32]>,
    input_types: Vec<Option<[u8; 32]>>,
    output_types: Vec<Option<[u8; 32]>>,
}

fn read_counts_from_transaction(transaction: &Cursor) -> Result<TxCounts, CoreError> {
    let tx = Transaction::from(transaction.clone());
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

fn tx_hash_from_syscalls() -> Result<[u8; 32], CoreError> {
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
    use alloc::vec;

    use crate::{error::CoreError, reader};

    #[test]
    fn reader_returns_cached_counts_transaction_and_tx_hash() {
        let counts = super::TxCounts {
            inputs: 1,
            outputs: 2,
            cell_deps: 3,
            header_deps: 4,
            witnesses: 5,
        };
        let transaction = reader::cursor_from_slice(&[4, 0, 0, 0]);
        let tx_hash = [9u8; 32];
        let reader = super::SyscallTxReader::from_cached_parts_for_tests(
            counts,
            transaction.clone(),
            tx_hash,
        );

        assert_eq!(reader.counts(), counts);
        assert_eq!(reader.tx_hash(), tx_hash);
        assert_eq!(
            reader::cursor_bytes_with_error(
                &reader.transaction_cursor(),
                CoreError::MissingHashInput
            )
            .unwrap(),
            vec![4, 0, 0, 0]
        );
    }
}
