use alloc::boxed::Box;
use core::cmp::min;

use ckb_std::{ckb_constants::Source, error::SysError, syscalls};
use cobuild_types::lazy_reader::support::{Cursor, Error as MoleculeError, Read};

#[derive(Clone, Copy)]
enum ChainReadTarget {
    Transaction,
    Script,
    ResolvedInputCell { index: usize },
    ResolvedInputData { index: usize },
}

impl ChainReadTarget {
    fn load(&self, buf: &mut [u8], offset: usize) -> Result<usize, SysError> {
        match *self {
            Self::Transaction => syscalls::load_transaction(buf, offset),
            Self::Script => syscalls::load_script(buf, offset),
            Self::ResolvedInputCell { index } => {
                syscalls::load_cell(buf, offset, index, Source::Input)
            }
            Self::ResolvedInputData { index } => {
                syscalls::load_cell_data(buf, offset, index, Source::Input)
            }
        }
    }
}

struct SourceBackedReader {
    total_size: usize,
    target: ChainReadTarget,
}

impl SourceBackedReader {
    fn new(target: ChainReadTarget) -> Result<Self, SysError> {
        let mut probe = [0u8; 1];
        let total_size = match target.load(&mut probe, 0) {
            Ok(size) => size,
            Err(SysError::LengthNotEnough(size)) => size,
            Err(err) => return Err(err),
        };
        Ok(Self { total_size, target })
    }
}

impl Read for SourceBackedReader {
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
            Err(err) => map_source_read_error(err, read_len),
        }
    }
}

fn map_source_read_error(err: SysError, read_len: usize) -> Result<usize, MoleculeError> {
    match err {
        SysError::LengthNotEnough(available) if available >= read_len => Ok(read_len),
        SysError::LengthNotEnough(available) => {
            Err(MoleculeError::Read(min(available, read_len), read_len))
        }
        _ => Err(MoleculeError::Read(0, read_len)),
    }
}

fn chain_cursor(target: ChainReadTarget) -> Result<Cursor, SysError> {
    let reader = SourceBackedReader::new(target)?;
    let total_size = reader.total_size;
    Ok(Cursor::new(total_size, Box::new(reader)))
}

pub(super) fn transaction_cursor() -> Result<Cursor, SysError> {
    chain_cursor(ChainReadTarget::Transaction)
}

pub(super) fn script_cursor() -> Result<Cursor, SysError> {
    chain_cursor(ChainReadTarget::Script)
}

pub(super) fn resolved_input_cell_cursor(index: usize) -> Result<Cursor, SysError> {
    chain_cursor(ChainReadTarget::ResolvedInputCell { index })
}

pub(super) fn resolved_input_data_cursor(index: usize) -> Result<Cursor, SysError> {
    chain_cursor(ChainReadTarget::ResolvedInputData { index })
}
