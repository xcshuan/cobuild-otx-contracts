use core::cell::Cell;

use cobuild_core::{
    engine::CobuildEngine,
    error::CoreError,
    reader::cursor_from_slice,
    source::{
        ClassifiedCursor, CursorReadContext, HashInputSource, InMemorySource, TransactionSource,
        TxCounts,
    },
};

#[test]
fn classified_cursor_read_error_matches_context() {
    let cursor = cursor_from_slice(&[]);

    assert_eq!(
        ClassifiedCursor::protocol(cursor.clone()).read_error(),
        CoreError::MalformedCobuild
    );
    assert_eq!(
        ClassifiedCursor::source_input(cursor.clone()).read_error(),
        CoreError::InvalidContextInput
    );
    assert_eq!(
        ClassifiedCursor::hash_input(cursor).read_error(),
        CoreError::MissingHashInput
    );
}

#[test]
fn in_memory_source_classifies_hash_payload_misses_as_missing_hash_input() {
    let source = InMemorySource::default();

    assert!(matches!(
        source.raw_input_cursor(0),
        Err(CoreError::MissingHashInput)
    ));
    assert!(matches!(
        source.raw_header_dep_hash(0),
        Err(CoreError::MissingHashInput)
    ));
}

#[test]
fn in_memory_source_classifies_source_and_hash_cursors() {
    let source = InMemorySource {
        transaction: vec![0x11],
        raw_inputs: vec![vec![0x22]],
        witnesses: vec![vec![0x33]],
        ..InMemorySource::default()
    };

    assert_eq!(
        source.transaction_cursor().unwrap().read_context,
        CursorReadContext::SourceInput
    );
    assert_eq!(
        source.raw_input_cursor(0).unwrap().read_context,
        CursorReadContext::HashInput
    );
    assert_eq!(
        source.witness_cursor(0).unwrap().read_context,
        CursorReadContext::HashInput
    );
}

#[test]
fn in_memory_source_exposes_counts_as_one_value() {
    let source = InMemorySource {
        raw_inputs: vec![Vec::new(); 2],
        raw_outputs: vec![Vec::new(); 1],
        raw_cell_deps: vec![Vec::new(); 3],
        raw_header_deps: vec![[0u8; 32]; 1],
        witnesses: vec![Vec::new(); 4],
        ..InMemorySource::default()
    };

    assert_eq!(
        source.counts().unwrap(),
        TxCounts {
            inputs: 2,
            outputs: 1,
            cell_deps: 3,
            header_deps: 1,
            witnesses: 4,
        }
    );
}

#[derive(Default)]
struct CountingSource {
    inner: InMemorySource,
    witness_reads: Cell<usize>,
}

impl TransactionSource for CountingSource {
    fn transaction_cursor(&self) -> Result<ClassifiedCursor, CoreError> {
        self.inner.transaction_cursor()
    }

    fn script_cursor(&self) -> Result<ClassifiedCursor, CoreError> {
        self.inner.script_cursor()
    }

    fn tx_hash(&self) -> Result<[u8; 32], CoreError> {
        self.inner.tx_hash()
    }

    fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        self.inner.input_lock_hash(index)
    }

    fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        self.inner.input_type_hash(index)
    }

    fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        self.inner.output_type_hash(index)
    }
}

impl HashInputSource for CountingSource {
    fn counts(&self) -> Result<TxCounts, CoreError> {
        self.inner.counts()
    }

    fn witness_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.witness_reads.set(self.witness_reads.get() + 1);
        self.inner.witness_cursor(index)
    }

    fn raw_input_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.inner.raw_input_cursor(index)
    }

    fn raw_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.inner.raw_output_cursor(index)
    }

    fn raw_output_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.inner.raw_output_data_cursor(index)
    }

    fn raw_cell_dep_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.inner.raw_cell_dep_cursor(index)
    }

    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        self.inner.raw_header_dep_hash(index)
    }

    fn resolved_input_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.inner.resolved_input_output_cursor(index)
    }

    fn resolved_input_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.inner.resolved_input_data_cursor(index)
    }
}

#[test]
fn engine_prepare_does_not_read_each_witness_twice() {
    let source = CountingSource {
        inner: InMemorySource {
            input_locks: vec![[1u8; 32]],
            input_types: vec![None],
            raw_inputs: vec![Vec::new()],
            witnesses: vec![Vec::new()],
            ..InMemorySource::default()
        },
        witness_reads: Cell::new(0),
    };

    let _prepared = CobuildEngine::prepare(&source).unwrap();

    assert_eq!(source.witness_reads.get(), 1);
}
