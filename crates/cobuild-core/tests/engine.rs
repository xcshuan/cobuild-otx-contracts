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
fn engine_prepare_defers_malformed_sighash_all_only_seal_for_absent_lock() {
    let source = InMemorySource {
        input_locks: vec![[1u8; 32]],
        input_types: vec![None],
        raw_inputs: vec![Vec::new()],
        witnesses: vec![malformed_sighash_all_only_witness()],
        ..InMemorySource::default()
    };

    let prepared = CobuildEngine::prepare(&source).unwrap();
    let plan = prepared.plan_lock_validation([2u8; 32], &source).unwrap();

    assert!(plan.required_signatures.is_empty());
}

#[test]
fn engine_lock_plan_rejects_malformed_sighash_all_only_carrier_when_relevant() {
    let source = InMemorySource {
        input_locks: vec![[1u8; 32]],
        input_types: vec![None],
        raw_inputs: vec![Vec::new()],
        witnesses: vec![malformed_sighash_all_only_witness()],
        ..InMemorySource::default()
    };

    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert!(matches!(
        prepared.plan_lock_validation([1u8; 32], &source),
        Err(CoreError::MalformedCobuild | CoreError::InvalidOtxLayout)
    ));
}

#[test]
fn engine_prepare_defers_malformed_sighash_all_body_for_absent_lock() {
    let source = InMemorySource {
        input_locks: vec![[1u8; 32]],
        input_types: vec![None],
        raw_inputs: vec![Vec::new()],
        witnesses: vec![malformed_sighash_all_witness()],
        ..InMemorySource::default()
    };

    let prepared = CobuildEngine::prepare(&source).unwrap();
    let plan = prepared.plan_lock_validation([2u8; 32], &source).unwrap();

    assert!(plan.required_signatures.is_empty());
}

#[test]
fn engine_lock_plan_rejects_malformed_sighash_all_carrier_when_relevant() {
    let source = InMemorySource {
        input_locks: vec![[1u8; 32]],
        input_types: vec![None],
        raw_inputs: vec![Vec::new()],
        witnesses: vec![malformed_sighash_all_witness()],
        ..InMemorySource::default()
    };

    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert!(matches!(
        prepared.plan_lock_validation([1u8; 32], &source),
        Err(CoreError::MalformedCobuild | CoreError::InvalidOtxLayout)
    ));
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
fn engine_lock_plan_rejects_message_action_target_absent_from_transaction() {
    let target_lock = [1u8; 32];
    let absent_output_type = [9u8; 32];
    let source = otx_source(
        vec![target_lock],
        vec![
            otx_start_witness(),
            otx_witness(
                &message_with_action(2, absent_output_type),
                &[seal_pair(target_lock, 0, &[7u8; 65])],
            ),
        ],
    );
    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert_eq!(
        prepared.plan_lock_validation(target_lock, &source),
        Err(CoreError::InvalidMessageTarget)
    );
}

#[test]
fn engine_lock_plan_rejects_duplicate_required_otx_seal_pair() {
    let target_lock = [1u8; 32];
    let source = otx_source(
        vec![target_lock],
        vec![
            otx_start_witness(),
            otx_witness(
                &empty_message(),
                &[
                    seal_pair(target_lock, 0, &[7u8; 65]),
                    seal_pair(target_lock, 0, &[7u8; 65]),
                    seal_pair(target_lock, 0, &[8u8; 65]),
                ],
            ),
        ],
    );
    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert_eq!(
        prepared.plan_lock_validation(target_lock, &source),
        Err(CoreError::DuplicateSealPair)
    );
}

#[test]
fn engine_lock_plan_rejects_invalid_otx_seal_scope() {
    let target_lock = [1u8; 32];
    let source = otx_source(
        vec![target_lock],
        vec![
            otx_start_witness(),
            otx_witness(&empty_message(), &[seal_pair(target_lock, 2, &[7u8; 65])]),
        ],
    );
    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert_eq!(
        prepared.plan_lock_validation(target_lock, &source),
        Err(CoreError::InvalidSealScope)
    );
}

#[test]
fn engine_lock_plan_rejects_uncovered_lock_group_without_tx_level_requirement() {
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

#[test]
fn engine_lock_plan_allows_combined_tx_level_and_otx_requirements() {
    let target_lock = [1u8; 32];
    let source = otx_source(
        vec![target_lock, target_lock],
        vec![
            sighash_all_only_witness(&[6u8; 65]),
            otx_start_witness_at(1),
            otx_witness(&empty_message(), &[seal_pair(target_lock, 0, &[7u8; 65])]),
        ],
    );
    let prepared = CobuildEngine::prepare(&source).unwrap();

    let plan = prepared.plan_lock_validation(target_lock, &source).unwrap();

    assert_eq!(plan.required_signatures.len(), 2);
    assert_eq!(plan.required_signatures[0].origin, SignatureOrigin::TxLevel);
    assert_eq!(plan.required_signatures[0].carrier_witness_index, 0);
    assert_eq!(plan.required_signatures[0].seal, vec![6u8; 65]);
    assert_eq!(plan.required_signatures[1].origin, SignatureOrigin::OtxBase);
    assert_eq!(plan.required_signatures[1].carrier_witness_index, 2);
    assert_eq!(plan.required_signatures[1].seal, vec![7u8; 65]);
}

#[test]
fn engine_lock_plan_for_unrelated_otx_lock_does_not_read_hash_payloads() {
    let target_lock = [1u8; 32];
    let unrelated_lock = [9u8; 32];
    let source = CountingSource::new(otx_source(
        vec![target_lock],
        vec![otx_start_witness(), otx_witness(&empty_message(), &[])],
    ));

    let prepared = CobuildEngine::prepare(&source).unwrap();
    let plan = prepared
        .plan_lock_validation(unrelated_lock, &source)
        .unwrap();

    assert!(plan.required_signatures.is_empty());
    assert_eq!(source.counters.resolved_outputs.get(), 0);
    assert_eq!(source.counters.resolved_data.get(), 0);
    assert_eq!(source.counters.raw_inputs.get(), 0);
    assert_eq!(source.counters.raw_outputs.get(), 0);
    assert_eq!(source.counters.raw_outputs_data.get(), 0);
    assert_eq!(source.counters.raw_cell_deps.get(), 0);
    assert_eq!(source.counters.raw_header_deps.get(), 0);
}

#[test]
fn engine_lock_plan_for_relevant_otx_lock_reads_hash_payloads() {
    let target_lock = [1u8; 32];
    let source = CountingSource::new(otx_source(
        vec![target_lock],
        vec![
            otx_start_witness(),
            otx_witness(&empty_message(), &[seal_pair(target_lock, 0, &[7u8; 65])]),
        ],
    ));

    let prepared = CobuildEngine::prepare(&source).unwrap();
    let plan = prepared.plan_lock_validation(target_lock, &source).unwrap();

    assert_eq!(plan.required_signatures.len(), 1);
    assert_eq!(plan.required_signatures[0].origin, SignatureOrigin::OtxBase);
    assert_eq!(source.counters.raw_inputs.get(), 1);
    assert_eq!(source.counters.resolved_outputs.get(), 1);
    assert_eq!(source.counters.resolved_data.get(), 1);
    assert_eq!(source.counters.raw_outputs.get(), 0);
    assert_eq!(source.counters.raw_outputs_data.get(), 0);
    assert_eq!(source.counters.raw_cell_deps.get(), 0);
    assert_eq!(source.counters.raw_header_deps.get(), 0);
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

#[derive(Default)]
struct ReadCounters {
    resolved_outputs: core::cell::Cell<usize>,
    resolved_data: core::cell::Cell<usize>,
    raw_inputs: core::cell::Cell<usize>,
    raw_outputs: core::cell::Cell<usize>,
    raw_outputs_data: core::cell::Cell<usize>,
    raw_cell_deps: core::cell::Cell<usize>,
    raw_header_deps: core::cell::Cell<usize>,
}

struct CountingSource {
    inner: InMemorySource,
    counters: ReadCounters,
}

impl CountingSource {
    fn new(inner: InMemorySource) -> Self {
        Self {
            inner,
            counters: ReadCounters::default(),
        }
    }
}

fn increment(counter: &core::cell::Cell<usize>) {
    counter.set(counter.get() + 1);
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

    fn witness_cursor(&self, absolute_index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.inner.witness_cursor(absolute_index)
    }

    fn raw_input_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        increment(&self.counters.raw_inputs);
        self.inner.raw_input_cursor(index)
    }

    fn raw_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        increment(&self.counters.raw_outputs);
        self.inner.raw_output_cursor(index)
    }

    fn raw_output_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        increment(&self.counters.raw_outputs_data);
        self.inner.raw_output_data_cursor(index)
    }

    fn raw_cell_dep_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        increment(&self.counters.raw_cell_deps);
        self.inner.raw_cell_dep_cursor(index)
    }

    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        increment(&self.counters.raw_header_deps);
        self.inner.raw_header_dep_hash(index)
    }

    fn resolved_input_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        increment(&self.counters.resolved_outputs);
        self.inner.resolved_input_output_cursor(index)
    }

    fn resolved_input_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        increment(&self.counters.resolved_data);
        self.inner.resolved_input_data_cursor(index)
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

fn malformed_sighash_all_only_witness() -> Vec<u8> {
    witness_union(0xff00_0002, &table(&[Vec::new()]))
}

fn malformed_sighash_all_witness() -> Vec<u8> {
    witness_union(0xff00_0001, &table(&[Vec::new()]))
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

fn message_with_action(script_role: u8, script_hash: [u8; 32]) -> Vec<u8> {
    table(&[dynvec(&[action(script_role, script_hash)])])
}

fn action(script_role: u8, script_hash: [u8; 32]) -> Vec<u8> {
    table(&[
        [0u8; 32].to_vec(),
        vec![script_role],
        script_hash.to_vec(),
        molecule_bytes(&[]),
    ])
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
    otx_start_witness_at(0)
}

fn otx_start_witness_at(start_input: u32) -> Vec<u8> {
    witness_union(
        0xff00_0004,
        &table(&[
            start_input.to_le_bytes().to_vec(),
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
