use cobuild_core::{
    context::{CobuildContext, TxScriptHashes},
    error::CoreError,
    hash::tx_with_message_hash,
    layout::LayoutTx,
    reader::cursor_from_slice,
    signature::SignatureOrigin,
    source::InMemorySource,
};

#[test]
fn lock_query_without_matching_lock_has_no_required_signatures() {
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: Vec::new(),
            input_count: 1,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![[1u8; 32]],
            input_types: vec![None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    let source = InMemorySource::default();
    assert!(context
        .lock_query([2u8; 32])
        .required_signatures(&source)
        .unwrap()
        .is_empty());
}

#[test]
fn lock_query_uses_current_lock_group_leading_witness_only() {
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: vec![Vec::new(), sighash_all_only_witness(&[7u8; 65])],
            input_count: 2,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![[1u8; 32], [2u8; 32]],
            input_types: vec![None, None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    let source = InMemorySource::default();

    assert!(context
        .lock_query([1u8; 32])
        .required_signatures(&source)
        .unwrap()
        .is_empty());

    let requests = context
        .lock_query([2u8; 32])
        .required_signatures(&source)
        .unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].origin, SignatureOrigin::SighashAll);
    assert_eq!(requests[0].carrier_witness_index, 1);
}

#[test]
fn lock_query_rejects_malformed_group_leading_witness() {
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: vec![vec![1, 2, 3, 4]],
            input_count: 1,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![[1u8; 32]],
            input_types: vec![None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    let source = InMemorySource::default();

    assert!(matches!(
        context.lock_query([1u8; 32]).required_signatures(&source),
        Err(CoreError::MalformedCobuild | CoreError::InvalidOtxLayout)
    ));
}

#[test]
fn sighash_all_only_uses_unique_sighash_all_message_hash() {
    let message = empty_message();
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: vec![
                sighash_all_only_witness(&[7u8; 65]),
                sighash_all_witness(&[8u8; 65], &message),
            ],
            input_count: 2,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![[1u8; 32], [2u8; 32]],
            input_types: vec![None, None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    let source = InMemorySource::default();

    let requests = context
        .lock_query([1u8; 32])
        .required_signatures(&source)
        .unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].origin, SignatureOrigin::SighashAll);
    assert_eq!(
        requests[0].signing_message_hash,
        tx_with_message_hash(&cursor_from_slice(&message), &source).unwrap()
    );
}

#[test]
fn lock_query_rejects_duplicate_sighash_all_witnesses() {
    let message = empty_message();
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: vec![
                sighash_all_witness(&[7u8; 65], &message),
                sighash_all_witness(&[8u8; 65], &message),
            ],
            input_count: 2,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![[1u8; 32], [2u8; 32]],
            input_types: vec![None, None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    let source = InMemorySource::default();

    assert_eq!(
        context.lock_query([1u8; 32]).required_signatures(&source),
        Err(CoreError::DuplicateSighashAll)
    );
}

#[test]
fn otx_signature_rejects_message_action_target_absent_from_transaction() {
    let target_lock = [1u8; 32];
    let absent_output_type = [9u8; 32];
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: vec![
                otx_start_witness(),
                otx_witness(
                    &message_with_action(2, absent_output_type),
                    &[seal_pair(target_lock, 0, &[7u8; 65])],
                ),
            ],
            input_count: 1,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![target_lock],
            input_types: vec![None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    let source = otx_signing_source(1);

    assert_eq!(
        context.lock_query(target_lock).required_signatures(&source),
        Err(CoreError::InvalidMessageTarget)
    );
}

#[test]
fn required_signatures_marks_otx_base_origin() {
    let target_lock = [1u8; 32];
    let context = otx_context(target_lock, &[seal_pair(target_lock, 0, &[7u8; 65])]);
    let source = otx_signing_source(1);

    let requests = context
        .lock_query(target_lock)
        .required_signatures(&source)
        .unwrap();

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].origin, SignatureOrigin::OtxBase);
    assert_eq!(requests[0].carrier_witness_index, 1);
}

#[test]
fn required_signatures_marks_otx_append_origin() {
    let target_lock = [1u8; 32];
    let base_lock = [2u8; 32];
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: vec![
                otx_start_witness(),
                otx_append_witness(&[seal_pair(target_lock, 1, &[7u8; 65])]),
            ],
            input_count: 2,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![base_lock, target_lock],
            input_types: vec![None, None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    let source = otx_signing_source(2);

    let requests = context
        .lock_query(target_lock)
        .required_signatures(&source)
        .unwrap();

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].origin, SignatureOrigin::OtxAppend);
    assert_eq!(requests[0].carrier_witness_index, 1);
}

#[test]
fn otx_signature_rejects_missing_required_seal_pair() {
    let target_lock = [1u8; 32];
    let context = otx_context(target_lock, &[]);
    let source = otx_signing_source(1);

    assert_eq!(
        context.lock_query(target_lock).required_signatures(&source),
        Err(CoreError::MissingSealPair)
    );
}

#[test]
fn otx_signature_rejects_duplicate_required_seal_pair() {
    let target_lock = [1u8; 32];
    let context = otx_context(
        target_lock,
        &[
            seal_pair(target_lock, 0, &[7u8; 65]),
            seal_pair(target_lock, 0, &[8u8; 65]),
        ],
    );
    let source = otx_signing_source(1);

    assert_eq!(
        context.lock_query(target_lock).required_signatures(&source),
        Err(CoreError::DuplicateSealPair)
    );
}

#[test]
fn otx_signature_rejects_invalid_seal_scope() {
    let target_lock = [1u8; 32];
    let context = otx_context(target_lock, &[seal_pair(target_lock, 2, &[7u8; 65])]);
    let source = otx_signing_source(1);

    assert_eq!(
        context.lock_query(target_lock).required_signatures(&source),
        Err(CoreError::InvalidSealScope)
    );
}

#[test]
fn otx_signature_rejects_invalid_message_action_role() {
    let target_lock = [1u8; 32];
    let context = otx_context_with_message(
        target_lock,
        &message_with_action(9, target_lock),
        &[seal_pair(target_lock, 0, &[7u8; 65])],
    );
    let source = otx_signing_source(1);

    assert_eq!(
        context.lock_query(target_lock).required_signatures(&source),
        Err(CoreError::InvalidMessageTarget)
    );
}

#[test]
fn unrelated_malformed_witness_does_not_force_cobuild_flow() {
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: vec![sighash_all_only_witness(&[7u8; 65]), vec![1, 2, 3, 4]],
            input_count: 2,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![[1u8; 32], [2u8; 32]],
            input_types: vec![None, None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    let source = InMemorySource::default();

    assert_eq!(
        context
            .lock_query([1u8; 32])
            .required_signatures(&source)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn unrelated_otx_lock_query_does_not_require_raw_hash_parts() {
    let target_lock = [1u8; 32];
    let unrelated_lock = [9u8; 32];
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: vec![otx_start_witness(), otx_witness(&empty_message(), &[])],
            input_count: 1,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![target_lock],
            input_types: vec![None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    let source = InMemorySource::default();

    assert_eq!(
        context
            .lock_query(unrelated_lock)
            .required_signatures(&source),
        Ok(Vec::new())
    );
}

#[test]
fn unrelated_malformed_otx_layout_does_not_fail_tx_level_lock_query() {
    let tx_lock = [1u8; 32];
    let otx_lock = [2u8; 32];
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: vec![
                sighash_all_only_witness(&[7u8; 65]),
                otx_start_witness_at(1),
                otx_witness_custom(&empty_message(), 0x10, 1, 0, &[]),
            ],
            input_count: 2,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![tx_lock, otx_lock],
            input_types: vec![None, None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    let source = InMemorySource::default();

    let requests = context
        .lock_query(tx_lock)
        .required_signatures(&source)
        .unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].origin, SignatureOrigin::SighashAll);
}

#[test]
fn otx_signature_rejects_uncovered_same_lock_remainder_input() {
    let target_lock = [1u8; 32];
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: vec![
                otx_start_witness(),
                otx_witness(&empty_message(), &[seal_pair(target_lock, 0, &[7u8; 65])]),
            ],
            input_count: 2,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![target_lock, target_lock],
            input_types: vec![None, None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    let source = otx_signing_source(2);

    assert_eq!(
        context.lock_query(target_lock).required_signatures(&source),
        Err(CoreError::MissingLockGroupCoverage)
    );
}

#[test]
fn duplicate_otx_start_after_tx_level_lock_does_not_fail_unrelated_lock_query() {
    let tx_lock = [1u8; 32];
    let otx_lock = [2u8; 32];
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: vec![
                sighash_all_only_witness(&[7u8; 65]),
                otx_start_witness_at(1),
                otx_start_witness_at(1),
            ],
            input_count: 2,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![tx_lock, otx_lock],
            input_types: vec![None, None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    let source = InMemorySource::default();

    let requests = context
        .lock_query(tx_lock)
        .required_signatures(&source)
        .unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].origin, SignatureOrigin::SighashAll);
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
    let bytes_size = 4 + seal.len() as u32;
    let table_size = 8 + bytes_size;
    let mut item = Vec::with_capacity(table_size as usize);
    item.extend_from_slice(&table_size.to_le_bytes());
    item.extend_from_slice(&8u32.to_le_bytes());
    item.extend_from_slice(&molecule_bytes(seal));
    item
}

fn molecule_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
    out
}

fn empty_message() -> Vec<u8> {
    let empty_action_vec = 4u32.to_le_bytes();
    let table_size = 8 + empty_action_vec.len() as u32;
    let mut message = Vec::with_capacity(table_size as usize);
    message.extend_from_slice(&table_size.to_le_bytes());
    message.extend_from_slice(&8u32.to_le_bytes());
    message.extend_from_slice(&empty_action_vec);
    message
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

fn otx_context(target_lock: [u8; 32], seals: &[Vec<u8>]) -> CobuildContext {
    otx_context_with_message(target_lock, &empty_message(), seals)
}

fn otx_context_with_message(
    target_lock: [u8; 32],
    message: &[u8],
    seals: &[Vec<u8>],
) -> CobuildContext {
    CobuildContext::new(
        LayoutTx {
            witnesses: vec![otx_start_witness(), otx_witness(message, seals)],
            input_count: 1,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![target_lock],
            input_types: vec![None],
            output_types: Vec::new(),
        },
    )
    .unwrap()
}

fn otx_signing_source(input_count: usize) -> InMemorySource {
    InMemorySource {
        tx_hash: [0u8; 32],
        raw_inputs: vec![Vec::new(); input_count],
        resolved_outputs: vec![Vec::new(); input_count],
        resolved_data: vec![Vec::new(); input_count],
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

fn table(fields: &[Vec<u8>]) -> Vec<u8> {
    let header_size = 4 + fields.len() * 4;
    let total_size = header_size + fields.iter().map(Vec::len).sum::<usize>();
    let mut out = Vec::with_capacity(total_size);
    out.extend_from_slice(&(total_size as u32).to_le_bytes());
    let mut offset = header_size;
    for field in fields {
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += field.len();
    }
    for field in fields {
        out.extend_from_slice(field);
    }
    out
}

fn dynvec(items: &[Vec<u8>]) -> Vec<u8> {
    if items.is_empty() {
        return 4u32.to_le_bytes().to_vec();
    }
    let header_size = 4 + items.len() * 4;
    let total_size = header_size + items.iter().map(Vec::len).sum::<usize>();
    let mut out = Vec::with_capacity(total_size);
    out.extend_from_slice(&(total_size as u32).to_le_bytes());
    let mut offset = header_size;
    for item in items {
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += item.len();
    }
    for item in items {
        out.extend_from_slice(item);
    }
    out
}
