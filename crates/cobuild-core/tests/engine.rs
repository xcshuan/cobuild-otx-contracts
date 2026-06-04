use cobuild_core::{
    engine::CobuildEngine,
    error::CoreError,
    hash::tx_without_message_hash,
    plan::SignatureOrigin,
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

#[test]
fn engine_lock_plan_uses_group_leading_sighash_all_only_witness() {
    let source = InMemorySource {
        input_locks: vec![[1u8; 32], [2u8; 32]],
        input_types: vec![None, None],
        raw_inputs: vec![Vec::new(); 2],
        resolved_outputs: vec![Vec::new(); 2],
        resolved_data: vec![Vec::new(); 2],
        witnesses: vec![Vec::new(), sighash_all_only_witness(&[7u8; 65])],
        ..InMemorySource::default()
    };

    let prepared = CobuildEngine::prepare(&source).unwrap();

    let lock_1_plan = prepared.plan_lock_validation([1u8; 32], &source).unwrap();
    assert!(lock_1_plan.required_signatures.is_empty());

    let lock_2_plan = prepared.plan_lock_validation([2u8; 32], &source).unwrap();
    assert_eq!(lock_2_plan.required_signatures.len(), 1);
    let requirement = &lock_2_plan.required_signatures[0];
    assert_eq!(requirement.origin, SignatureOrigin::TxLevel);
    assert_eq!(requirement.carrier_witness_index, 1);
    assert_eq!(requirement.seal, vec![7u8; 65]);
    assert_eq!(
        requirement.signing_message_hash,
        tx_without_message_hash(&source).unwrap()
    );
}

#[test]
fn engine_lock_plan_rejects_duplicate_sighash_all_when_tx_level_relevant() {
    let message = empty_message();
    let source = InMemorySource {
        input_locks: vec![[1u8; 32], [2u8; 32]],
        input_types: vec![None, None],
        raw_inputs: vec![Vec::new(); 2],
        resolved_outputs: vec![Vec::new(); 2],
        resolved_data: vec![Vec::new(); 2],
        witnesses: vec![
            sighash_all_witness(&[7u8; 65], &message),
            sighash_all_witness(&[8u8; 65], &message),
        ],
        ..InMemorySource::default()
    };

    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert_eq!(
        prepared.plan_lock_validation([1u8; 32], &source),
        Err(CoreError::DuplicateSighashAll)
    );
}

#[test]
fn engine_lock_plan_marks_otx_base_requirement() {
    let target_lock = [1u8; 32];
    let source = otx_source(
        vec![target_lock],
        vec![
            otx_start_witness(),
            otx_witness(&empty_message(), &[seal_pair(target_lock, 0, &[7u8; 65])]),
        ],
    );
    let prepared = CobuildEngine::prepare(&source).unwrap();

    let plan = prepared.plan_lock_validation(target_lock, &source).unwrap();

    assert_eq!(plan.required_signatures.len(), 1);
    assert_eq!(plan.required_signatures[0].origin, SignatureOrigin::OtxBase);
    assert_eq!(plan.required_signatures[0].carrier_witness_index, 1);
}

#[test]
fn engine_lock_plan_marks_otx_append_requirement() {
    let target_lock = [1u8; 32];
    let base_lock = [2u8; 32];
    let source = otx_source(
        vec![base_lock, target_lock],
        vec![
            otx_start_witness(),
            otx_append_witness(&[seal_pair(target_lock, 1, &[7u8; 65])]),
        ],
    );
    let prepared = CobuildEngine::prepare(&source).unwrap();

    let plan = prepared.plan_lock_validation(target_lock, &source).unwrap();

    assert_eq!(plan.required_signatures.len(), 1);
    assert_eq!(
        plan.required_signatures[0].origin,
        SignatureOrigin::OtxAppend
    );
    assert_eq!(plan.required_signatures[0].carrier_witness_index, 1);
}

#[test]
fn engine_lock_plan_rejects_missing_otx_seal_for_relevant_scope() {
    let target_lock = [1u8; 32];
    let source = otx_source(
        vec![target_lock],
        vec![otx_start_witness(), otx_witness(&empty_message(), &[])],
    );
    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert_eq!(
        prepared.plan_lock_validation(target_lock, &source),
        Err(CoreError::MissingSealPair)
    );
}

#[test]
fn engine_lock_plan_rejects_uncovered_same_lock_remainder_input() {
    let target_lock = [1u8; 32];
    let source = otx_source(
        vec![target_lock, target_lock],
        vec![
            otx_start_witness(),
            otx_witness(&empty_message(), &[seal_pair(target_lock, 0, &[7u8; 65])]),
        ],
    );
    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert_eq!(
        prepared.plan_lock_validation(target_lock, &source),
        Err(CoreError::MissingLockGroupCoverage)
    );
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

fn sighash_all_only_witness(seal: &[u8]) -> Vec<u8> {
    const SIGHASH_ALL_ONLY_ID: u32 = 0xff00_0002;

    let item = sighash_all_only_table(seal);
    let mut witness = Vec::with_capacity(4 + item.len());
    witness.extend_from_slice(&SIGHASH_ALL_ONLY_ID.to_le_bytes());
    witness.extend_from_slice(&item);
    witness
}

fn sighash_all_witness(seal: &[u8], message: &[u8]) -> Vec<u8> {
    const SIGHASH_ALL_ID: u32 = 0xff00_0001;

    let seal_bytes = molecule_bytes(seal);
    let table_size = 12 + seal_bytes.len() as u32 + message.len() as u32;
    let mut item = Vec::with_capacity(table_size as usize);
    item.extend_from_slice(&table_size.to_le_bytes());
    item.extend_from_slice(&12u32.to_le_bytes());
    item.extend_from_slice(&(12 + seal_bytes.len() as u32).to_le_bytes());
    item.extend_from_slice(&seal_bytes);
    item.extend_from_slice(message);

    let mut witness = Vec::with_capacity(4 + item.len());
    witness.extend_from_slice(&SIGHASH_ALL_ID.to_le_bytes());
    witness.extend_from_slice(&item);
    witness
}

fn sighash_all_only_table(seal: &[u8]) -> Vec<u8> {
    table(&[molecule_bytes(seal)])
}

fn empty_message() -> Vec<u8> {
    table(&[dynvec(&[])])
}

fn otx_source(input_locks: Vec<[u8; 32]>, witnesses: Vec<Vec<u8>>) -> InMemorySource {
    let input_count = input_locks.len();
    InMemorySource {
        input_locks,
        input_types: vec![None; input_count],
        output_types: Vec::new(),
        raw_inputs: vec![Vec::new(); input_count],
        resolved_outputs: vec![Vec::new(); input_count],
        resolved_data: vec![Vec::new(); input_count],
        witnesses,
        ..InMemorySource::default()
    }
}

fn otx_start_witness() -> Vec<u8> {
    witness_union(
        0xff00_0004,
        &table(&[
            0u32.to_le_bytes().to_vec(),
            0u32.to_le_bytes().to_vec(),
            0u32.to_le_bytes().to_vec(),
            0u32.to_le_bytes().to_vec(),
        ]),
    )
}

fn otx_witness(message: &[u8], seals: &[Vec<u8>]) -> Vec<u8> {
    otx_witness_custom(message, 0, 1, 0, seals)
}

fn otx_append_witness(seals: &[Vec<u8>]) -> Vec<u8> {
    otx_witness_custom(&empty_message(), 0x01, 1, 1, seals)
}

fn otx_witness_custom(
    message: &[u8],
    append_permissions: u8,
    base_input_cells: u32,
    append_input_cells: u32,
    seals: &[Vec<u8>],
) -> Vec<u8> {
    witness_union(
        0xff00_0003,
        &table(&[
            message.to_vec(),
            vec![append_permissions],
            base_input_cells.to_le_bytes().to_vec(),
            molecule_bytes(&[0]),
            0u32.to_le_bytes().to_vec(),
            molecule_bytes(&[]),
            0u32.to_le_bytes().to_vec(),
            molecule_bytes(&[]),
            0u32.to_le_bytes().to_vec(),
            molecule_bytes(&[]),
            append_input_cells.to_le_bytes().to_vec(),
            0u32.to_le_bytes().to_vec(),
            0u32.to_le_bytes().to_vec(),
            0u32.to_le_bytes().to_vec(),
            dynvec(seals),
        ]),
    )
}

fn seal_pair(script_hash: [u8; 32], scope: u8, seal: &[u8]) -> Vec<u8> {
    table(&[script_hash.to_vec(), vec![scope], molecule_bytes(seal)])
}

fn witness_union(item_id: u32, item: &[u8]) -> Vec<u8> {
    let mut witness = Vec::with_capacity(4 + item.len());
    witness.extend_from_slice(&item_id.to_le_bytes());
    witness.extend_from_slice(item);
    witness
}

fn molecule_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
    out
}

fn dynvec(items: &[Vec<u8>]) -> Vec<u8> {
    let header_size = 4 + items.len() * 4;
    let total_size = header_size + items.iter().map(Vec::len).sum::<usize>();
    let mut out = Vec::with_capacity(total_size);
    out.extend_from_slice(&(total_size as u32).to_le_bytes());
    let mut offset = header_size as u32;
    for item in items {
        out.extend_from_slice(&offset.to_le_bytes());
        offset += item.len() as u32;
    }
    for item in items {
        out.extend_from_slice(item);
    }
    out
}

fn table(fields: &[Vec<u8>]) -> Vec<u8> {
    let header_size = 4 + fields.len() * 4;
    let total_size = header_size + fields.iter().map(Vec::len).sum::<usize>();
    let mut out = Vec::with_capacity(total_size);
    out.extend_from_slice(&(total_size as u32).to_le_bytes());
    let mut offset = header_size as u32;
    for field in fields {
        out.extend_from_slice(&offset.to_le_bytes());
        offset += field.len() as u32;
    }
    for field in fields {
        out.extend_from_slice(field);
    }
    out
}
