use alloc::vec::Vec;

use cobuild_types::lazy_reader::support::Cursor;

use crate::{error::CoreError, reader::cursor_from_slice};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorReadContext {
    Protocol,
    SourceInput,
    HashInput,
}

#[derive(Clone)]
pub struct ClassifiedCursor {
    pub cursor: Cursor,
    pub read_context: CursorReadContext,
}

impl ClassifiedCursor {
    pub fn new(cursor: Cursor, read_context: CursorReadContext) -> Self {
        Self {
            cursor,
            read_context,
        }
    }

    pub fn protocol(cursor: Cursor) -> Self {
        Self::new(cursor, CursorReadContext::Protocol)
    }

    pub fn source_input(cursor: Cursor) -> Self {
        Self::new(cursor, CursorReadContext::SourceInput)
    }

    pub fn hash_input(cursor: Cursor) -> Self {
        Self::new(cursor, CursorReadContext::HashInput)
    }

    pub fn read_error(&self) -> CoreError {
        match self.read_context {
            CursorReadContext::Protocol => CoreError::MalformedCobuild,
            CursorReadContext::SourceInput => CoreError::InvalidContextInput,
            CursorReadContext::HashInput => CoreError::MissingHashInput,
        }
    }
}

pub trait TransactionSource {
    fn transaction_cursor(&self) -> Result<ClassifiedCursor, CoreError>;
    fn script_cursor(&self) -> Result<ClassifiedCursor, CoreError>;
    fn tx_hash(&self) -> Result<[u8; 32], CoreError>;
    fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError>;
    fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError>;
    fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError>;
    fn resolved_input_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn resolved_input_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
}

pub trait SigningDataSource: TransactionSource {
    fn input_count(&self) -> Result<usize, CoreError>;
    fn witness_count(&self) -> Result<usize, CoreError>;
    fn witness_cursor(&self, absolute_index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_input_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_output_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_cell_dep_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemorySource {
    pub transaction: Vec<u8>,
    pub script: Vec<u8>,
    pub tx_hash: [u8; 32],
    pub input_locks: Vec<[u8; 32]>,
    pub input_types: Vec<Option<[u8; 32]>>,
    pub output_types: Vec<Option<[u8; 32]>>,
    pub resolved_outputs: Vec<Vec<u8>>,
    pub resolved_data: Vec<Vec<u8>>,
    pub raw_inputs: Vec<Vec<u8>>,
    pub raw_outputs: Vec<Vec<u8>>,
    pub raw_outputs_data: Vec<Vec<u8>>,
    pub raw_cell_deps: Vec<Vec<u8>>,
    pub raw_header_deps: Vec<[u8; 32]>,
    pub witnesses: Vec<Vec<u8>>,
}

fn hash_input_cursor(items: &[Vec<u8>], index: usize) -> Result<ClassifiedCursor, CoreError> {
    let item = items.get(index).ok_or(CoreError::MissingHashInput)?;
    Ok(ClassifiedCursor::hash_input(cursor_from_slice(item)))
}

impl TransactionSource for InMemorySource {
    fn transaction_cursor(&self) -> Result<ClassifiedCursor, CoreError> {
        Ok(ClassifiedCursor::source_input(cursor_from_slice(
            &self.transaction,
        )))
    }

    fn script_cursor(&self) -> Result<ClassifiedCursor, CoreError> {
        Ok(ClassifiedCursor::source_input(cursor_from_slice(
            &self.script,
        )))
    }

    fn tx_hash(&self) -> Result<[u8; 32], CoreError> {
        Ok(self.tx_hash)
    }

    fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        self.input_locks
            .get(index)
            .copied()
            .ok_or(CoreError::InvalidContextInput)
    }

    fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        self.input_types
            .get(index)
            .copied()
            .ok_or(CoreError::InvalidContextInput)
    }

    fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        self.output_types
            .get(index)
            .copied()
            .ok_or(CoreError::InvalidContextInput)
    }

    fn resolved_input_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_input_cursor(&self.resolved_outputs, index)
    }

    fn resolved_input_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_input_cursor(&self.resolved_data, index)
    }
}

impl SigningDataSource for InMemorySource {
    fn input_count(&self) -> Result<usize, CoreError> {
        Ok(self.raw_inputs.len())
    }

    fn witness_count(&self) -> Result<usize, CoreError> {
        Ok(self.witnesses.len())
    }

    fn witness_cursor(&self, absolute_index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_input_cursor(&self.witnesses, absolute_index)
    }

    fn raw_input_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_input_cursor(&self.raw_inputs, index)
    }

    fn raw_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_input_cursor(&self.raw_outputs, index)
    }

    fn raw_output_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_input_cursor(&self.raw_outputs_data, index)
    }

    fn raw_cell_dep_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_input_cursor(&self.raw_cell_deps, index)
    }

    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        self.raw_header_deps
            .get(index)
            .copied()
            .ok_or(CoreError::MissingHashInput)
    }
}
