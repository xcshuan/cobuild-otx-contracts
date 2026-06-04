use cobuild_core::{engine::CobuildEngine, plan::MessageOrigin, source::InMemorySource};

#[test]
fn type_plan_exposes_tx_level_message_for_related_input_type() {
    let type_hash = [3u8; 32];
    let message = empty_message();
    let source = InMemorySource {
        input_locks: vec![[1u8; 32]],
        input_types: vec![Some(type_hash)],
        output_types: Vec::new(),
        raw_inputs: vec![Vec::new()],
        witnesses: vec![sighash_all_witness(&[7u8; 65], &message)],
        ..InMemorySource::default()
    };
    let prepared = CobuildEngine::prepare(&source).unwrap();

    let plan = prepared.plan_type_validation(type_hash, &source).unwrap();

    assert_eq!(plan.type_script_hash, type_hash);
    assert_eq!(plan.related_messages.len(), 1);
    assert!(matches!(
        plan.related_messages[0].origin,
        MessageOrigin::TxLevel {
            carrier_witness_index: 0
        }
    ));
}

#[test]
fn type_plan_exposes_otx_message_with_relation_flags() {
    let type_hash = [3u8; 32];
    let target_lock = [1u8; 32];
    let source = InMemorySource {
        input_locks: vec![target_lock],
        input_types: vec![Some(type_hash)],
        output_types: Vec::new(),
        raw_inputs: vec![Vec::new()],
        resolved_outputs: vec![Vec::new()],
        resolved_data: vec![Vec::new()],
        witnesses: vec![
            otx_start_witness(),
            otx_witness(&empty_message(), &[seal_pair(target_lock, 0, &[7u8; 65])]),
        ],
        ..InMemorySource::default()
    };
    let prepared = CobuildEngine::prepare(&source).unwrap();

    let plan = prepared.plan_type_validation(type_hash, &source).unwrap();

    assert_eq!(plan.related_messages.len(), 1);
    match plan.related_messages[0].origin {
        MessageOrigin::Otx {
            witness_index,
            otx_index,
            relation,
            ..
        } => {
            assert_eq!(witness_index, 1);
            assert_eq!(otx_index, 0);
            assert!(relation.input_type_in_base);
            assert!(!relation.input_type_in_append);
        }
        MessageOrigin::TxLevel { .. } => panic!("expected otx message"),
    }
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

fn empty_message() -> Vec<u8> {
    table(&[dynvec(&[])])
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
