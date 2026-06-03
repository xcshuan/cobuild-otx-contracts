use cobuild_types::lazy_reader::support::Cursor;

use crate::error::CoreError;

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
    fn header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError>;
}
