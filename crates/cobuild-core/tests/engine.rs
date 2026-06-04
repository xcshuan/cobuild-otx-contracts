use cobuild_core::{
    engine::CobuildEngine,
    error::CoreError,
    source::{ClassifiedCursor, HashInputSource, InMemorySource, TransactionSource, TxCounts},
};
use cobuild_types::lazy_reader::support::{Cursor, Error as MoleculeError, Read};

#[test]
fn engine_returns_empty_lock_plan_when_lock_is_absent() {
    let source = InMemorySource {
        input_locks: vec![[1u8; 32]],
        input_types: vec![None],
        raw_inputs: vec![Vec::new()],
        ..InMemorySource::default()
    };

    let prepared = CobuildEngine::prepare(&source).unwrap();
    let plan = prepared.plan_lock_validation([2u8; 32], &source).unwrap();

    assert_eq!(plan.lock_script_hash, [2u8; 32]);
    assert!(plan.required_signatures.is_empty());
}

#[test]
fn engine_preparation_uses_source_counts() {
    let source = InMemorySource {
        input_locks: vec![[1u8; 32], [2u8; 32], [3u8; 32]],
        input_types: vec![Some([4u8; 32]), None, Some([5u8; 32])],
        output_types: vec![None, Some([6u8; 32])],
        raw_inputs: vec![Vec::new(); 3],
        raw_outputs: vec![Vec::new(); 2],
        raw_cell_deps: vec![Vec::new(); 1],
        raw_header_deps: vec![[7u8; 32]; 2],
        witnesses: vec![Vec::new(); 4],
        ..InMemorySource::default()
    };

    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert_eq!(
        prepared.counts(),
        TxCounts {
            inputs: 3,
            outputs: 2,
            cell_deps: 1,
            header_deps: 2,
            witnesses: 4,
        }
    );
}

#[test]
fn engine_preparation_classifies_witness_read_errors_as_missing_hash_input() {
    let result = CobuildEngine::prepare(&FailingWitnessSource);

    assert!(matches!(result, Err(CoreError::MissingHashInput)));
}

struct FailingWitnessSource;

impl TransactionSource for FailingWitnessSource {
    fn transaction_cursor(&self) -> Result<ClassifiedCursor, CoreError> {
        Err(CoreError::InvalidContextInput)
    }

    fn script_cursor(&self) -> Result<ClassifiedCursor, CoreError> {
        Err(CoreError::InvalidContextInput)
    }

    fn tx_hash(&self) -> Result<[u8; 32], CoreError> {
        Err(CoreError::InvalidContextInput)
    }

    fn input_lock_hash(&self, _index: usize) -> Result<[u8; 32], CoreError> {
        Err(CoreError::InvalidContextInput)
    }

    fn input_type_hash(&self, _index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        Err(CoreError::InvalidContextInput)
    }

    fn output_type_hash(&self, _index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        Err(CoreError::InvalidContextInput)
    }
}

impl HashInputSource for FailingWitnessSource {
    fn counts(&self) -> Result<TxCounts, CoreError> {
        Ok(TxCounts {
            witnesses: 1,
            ..TxCounts::default()
        })
    }

    fn witness_cursor(&self, _absolute_index: usize) -> Result<ClassifiedCursor, CoreError> {
        Ok(ClassifiedCursor::hash_input(Cursor::new(
            1,
            Box::new(FailingReader),
        )))
    }

    fn raw_input_cursor(&self, _index: usize) -> Result<ClassifiedCursor, CoreError> {
        Err(CoreError::MissingHashInput)
    }

    fn raw_output_cursor(&self, _index: usize) -> Result<ClassifiedCursor, CoreError> {
        Err(CoreError::MissingHashInput)
    }

    fn raw_output_data_cursor(&self, _index: usize) -> Result<ClassifiedCursor, CoreError> {
        Err(CoreError::MissingHashInput)
    }

    fn raw_cell_dep_cursor(&self, _index: usize) -> Result<ClassifiedCursor, CoreError> {
        Err(CoreError::MissingHashInput)
    }

    fn raw_header_dep_hash(&self, _index: usize) -> Result<[u8; 32], CoreError> {
        Err(CoreError::MissingHashInput)
    }

    fn resolved_input_output_cursor(&self, _index: usize) -> Result<ClassifiedCursor, CoreError> {
        Err(CoreError::MissingHashInput)
    }

    fn resolved_input_data_cursor(&self, _index: usize) -> Result<ClassifiedCursor, CoreError> {
        Err(CoreError::MissingHashInput)
    }
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&self, _buf: &mut [u8], _offset: usize) -> Result<usize, MoleculeError> {
        Err(MoleculeError::OutOfBound(0, 1))
    }
}
