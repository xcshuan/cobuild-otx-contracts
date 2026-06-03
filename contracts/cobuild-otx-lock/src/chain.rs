use alloc::{boxed::Box, vec, vec::Vec};
use core::cmp::min;

use ckb_std::{
    ckb_constants::{CellField, Source},
    error::SysError,
    syscalls,
};
use cobuild_core::{
    error::CoreError,
    prepare::{SourcePreparedContext, prepare_context_from_source, script_args_from_slice},
    source::{ClassifiedCursor, SigningDataSource, TransactionSource},
};
use cobuild_types::lazy_reader::{
    blockchain::Transaction,
    support::{Cursor, Error as MoleculeError, Read},
};

use crate::{
    error::Error,
    errors::{map_core_error, map_sys_error},
};

pub(crate) fn load_current_script_args() -> Result<Vec<u8>, Error> {
    script_args_from_slice(&load_script()?).map_err(map_core_error)
}

pub(crate) struct LoadedContext {
    pub source: ChainSource,
    pub prepared: SourcePreparedContext,
}

pub(crate) fn load_prepared_context() -> Result<LoadedContext, Error> {
    let source = ChainSource;
    let prepared = prepare_context_from_source(&source).map_err(map_core_error)?;
    Ok(LoadedContext { source, prepared })
}

pub(crate) struct ChainSource;

#[derive(Clone, Copy)]
enum SyscallTarget {
    Transaction,
    Script,
    Cell { index: usize, source: Source },
    CellData { index: usize, source: Source },
}

impl SyscallTarget {
    fn load(&self, buf: &mut [u8], offset: usize) -> Result<usize, SysError> {
        match *self {
            Self::Transaction => syscalls::load_transaction(buf, offset),
            Self::Script => syscalls::load_script(buf, offset),
            Self::Cell { index, source } => syscalls::load_cell(buf, offset, index, source),
            Self::CellData { index, source } => {
                syscalls::load_cell_data(buf, offset, index, source)
            }
        }
    }
}

struct SyscallReader {
    total_size: usize,
    target: SyscallTarget,
}

impl SyscallReader {
    fn new(target: SyscallTarget) -> Result<Self, SysError> {
        let mut probe = [0u8; 1];
        let total_size = match target.load(&mut probe, 0) {
            Ok(size) => size,
            Err(SysError::LengthNotEnough(size)) => size,
            Err(err) => return Err(err),
        };
        Ok(Self { total_size, target })
    }
}

impl Read for SyscallReader {
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
            Err(SysError::LengthNotEnough(available)) if available >= read_len => Ok(read_len),
            Err(SysError::LengthNotEnough(available)) => {
                Err(MoleculeError::Read(min(available, read_len), read_len))
            }
            Err(_) => Err(MoleculeError::Read(0, read_len)),
        }
    }
}

fn syscall_cursor(target: SyscallTarget) -> Result<Cursor, SysError> {
    let reader = SyscallReader::new(target)?;
    let total_size = reader.total_size;
    Ok(Cursor::new(total_size, Box::new(reader)))
}

fn load_owned(target: SyscallTarget) -> Result<Vec<u8>, Error> {
    let cursor = syscall_cursor(target).map_err(map_sys_error)?;
    let mut data = vec![0; cursor.size];
    let read = cursor
        .read_at(&mut data)
        .map_err(|_| Error::SyscallFailure)?;
    if read != data.len() {
        return Err(Error::SyscallFailure);
    }
    Ok(data)
}

fn source_cursor(target: SyscallTarget) -> Result<ClassifiedCursor, CoreError> {
    syscall_cursor(target)
        .map(ClassifiedCursor::source_input)
        .map_err(|_| CoreError::InvalidContextInput)
}

fn hash_cursor(target: SyscallTarget) -> Result<ClassifiedCursor, CoreError> {
    syscall_cursor(target)
        .map(ClassifiedCursor::hash_input)
        .map_err(|_| CoreError::MissingHashInput)
}

fn load_tx_hash() -> Result<[u8; 32], SysError> {
    let mut hash = [0u8; 32];
    syscalls::load_tx_hash(&mut hash, 0)?;
    Ok(hash)
}

pub(crate) fn load_script_hash() -> Result<[u8; 32], Error> {
    let mut hash = [0u8; 32];
    syscalls::load_script_hash(&mut hash, 0).map_err(map_sys_error)?;
    Ok(hash)
}

fn load_script() -> Result<Vec<u8>, Error> {
    load_owned(SyscallTarget::Script)
}

fn load_cell_field_hash(
    index: usize,
    source: Source,
    field: CellField,
) -> Result<[u8; 32], SysError> {
    let mut hash = [0u8; 32];
    syscalls::load_cell_by_field(&mut hash, 0, index, source, field)?;
    Ok(hash)
}

fn source_type_hash(index: usize, source: Source) -> Result<Option<[u8; 32]>, CoreError> {
    match load_cell_field_hash(index, source, CellField::TypeHash) {
        Ok(hash) => Ok(Some(hash)),
        Err(SysError::ItemMissing) => Ok(None),
        Err(_) => Err(CoreError::InvalidContextInput),
    }
}

fn hash_transaction() -> Result<Transaction, CoreError> {
    let cursor =
        syscall_cursor(SyscallTarget::Transaction).map_err(|_| CoreError::MissingHashInput)?;
    Ok(Transaction::from(cursor))
}

impl TransactionSource for ChainSource {
    fn transaction_cursor(&self) -> Result<ClassifiedCursor, CoreError> {
        source_cursor(SyscallTarget::Transaction)
    }

    fn script_cursor(&self) -> Result<ClassifiedCursor, CoreError> {
        source_cursor(SyscallTarget::Script)
    }

    fn tx_hash(&self) -> Result<[u8; 32], CoreError> {
        load_tx_hash().map_err(|_| CoreError::InvalidContextInput)
    }

    fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        load_cell_field_hash(index, Source::Input, CellField::LockHash)
            .map_err(|_| CoreError::InvalidContextInput)
    }

    fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        source_type_hash(index, Source::Input)
    }

    fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        source_type_hash(index, Source::Output)
    }

    fn resolved_input_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_cursor(SyscallTarget::Cell {
            index,
            source: Source::Input,
        })
    }

    fn resolved_input_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_cursor(SyscallTarget::CellData {
            index,
            source: Source::Input,
        })
    }
}

impl SigningDataSource for ChainSource {
    fn input_count(&self) -> Result<usize, CoreError> {
        hash_transaction()?
            .raw()
            .and_then(|raw| raw.inputs())
            .and_then(|inputs| inputs.len())
            .map_err(|_| CoreError::MissingHashInput)
    }

    fn witness_count(&self) -> Result<usize, CoreError> {
        hash_transaction()?
            .witnesses()
            .and_then(|witnesses| witnesses.len())
            .map_err(|_| CoreError::MissingHashInput)
    }

    fn witness_cursor(&self, absolute_index: usize) -> Result<ClassifiedCursor, CoreError> {
        let cursor = hash_transaction()?
            .witnesses()
            .and_then(|witnesses| witnesses.get(absolute_index))
            .map_err(|_| CoreError::MissingHashInput)?;
        Ok(ClassifiedCursor::hash_input(cursor))
    }

    fn raw_input_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        let input = hash_transaction()?
            .raw()
            .and_then(|raw| raw.inputs())
            .and_then(|inputs| inputs.get(index))
            .map_err(|_| CoreError::MissingHashInput)?;
        Ok(ClassifiedCursor::hash_input(input.cursor))
    }

    fn raw_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        let output = hash_transaction()?
            .raw()
            .and_then(|raw| raw.outputs())
            .and_then(|outputs| outputs.get(index))
            .map_err(|_| CoreError::MissingHashInput)?;
        Ok(ClassifiedCursor::hash_input(output.cursor))
    }

    fn raw_output_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        let cursor = hash_transaction()?
            .raw()
            .and_then(|raw| raw.outputs_data())
            .and_then(|outputs_data| outputs_data.get(index))
            .map_err(|_| CoreError::MissingHashInput)?;
        Ok(ClassifiedCursor::hash_input(cursor))
    }

    fn raw_cell_dep_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        let cell_dep = hash_transaction()?
            .raw()
            .and_then(|raw| raw.cell_deps())
            .and_then(|cell_deps| cell_deps.get(index))
            .map_err(|_| CoreError::MissingHashInput)?;
        Ok(ClassifiedCursor::hash_input(cell_dep.cursor))
    }

    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        hash_transaction()?
            .raw()
            .and_then(|raw| raw.header_deps())
            .and_then(|header_deps| header_deps.get(index))
            .map_err(|_| CoreError::MissingHashInput)
    }
}
