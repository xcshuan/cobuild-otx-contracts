use cobuild_core::{
    error::CoreError,
    hash::{
        checked_len_prefix, otx_base_hash, tx_without_message_hash, RawTxParts,
        ResolvedInputHashPart, SigningHashParts,
    },
    layout::{OtxLayout, Range},
    reader::OwnedReader,
    view::OtxData,
};
use cobuild_types::lazy_reader::{
    blockchain::{CellInput, CellOutput},
    support::Cursor,
};

#[test]
fn tx_without_message_hash_is_deterministic() {
    let parts = SigningHashParts {
        tx_hash: [7u8; 32],
        resolved_inputs: Vec::new(),
        trailing_witnesses: Vec::new(),
    };
    assert_eq!(
        tx_without_message_hash(&parts).unwrap(),
        tx_without_message_hash(&parts).unwrap()
    );
}

#[test]
fn len_prefix_rejects_values_larger_than_u32() {
    assert_eq!(
        checked_len_prefix((u32::MAX as usize) + 1),
        Err(CoreError::HashInputTooLarge)
    );
}

#[test]
fn resolved_input_output_is_not_length_prefixed() {
    let parts = SigningHashParts {
        tx_hash: [9u8; 32],
        resolved_inputs: vec![ResolvedInputHashPart {
            output: vec![1, 2, 3],
            data: vec![4, 5],
        }],
        trailing_witnesses: vec![vec![6, 7, 8]],
    };

    let mut expected = [0u8; 32];
    let mut hasher = blake2b_ref::Blake2bBuilder::new(32)
        .personal(b"ckbcb_tnm_core1\0")
        .build();
    hasher.update(&parts.tx_hash);
    hasher.update(&[1, 2, 3]);
    hasher.update(&(2u32.to_le_bytes()));
    hasher.update(&[4, 5]);
    hasher.update(&(3u32.to_le_bytes()));
    hasher.update(&[6, 7, 8]);
    hasher.finalize(&mut expected);

    assert_eq!(tx_without_message_hash(&parts).unwrap(), expected);
}

#[test]
fn otx_base_hash_includes_local_indices_for_base_deps_and_headers() {
    let otx = OtxData {
        message: vec![0x11],
        append_permissions: 0,
        base_input_cells: 0,
        base_input_masks: Vec::new(),
        base_output_cells: 0,
        base_output_masks: Vec::new(),
        base_cell_deps: 1,
        base_cell_dep_masks: vec![1],
        base_header_deps: 1,
        base_header_dep_masks: vec![1],
        append_input_cells: 0,
        append_output_cells: 0,
        append_cell_deps: 0,
        append_header_deps: 0,
        seals: Vec::new(),
    };
    let layout = OtxLayout {
        witness_index: 0,
        base_inputs: range(0, 0),
        append_inputs: range(0, 0),
        base_outputs: range(0, 0),
        append_outputs: range(0, 0),
        base_cell_deps: range(0, 1),
        append_cell_deps: range(1, 0),
        base_header_deps: range(0, 1),
        append_header_deps: range(1, 0),
    };
    let raw = RawTxParts {
        cell_deps: vec![vec![0x22]],
        header_deps: vec![[0x33; 32]],
        ..RawTxParts::default()
    };

    let actual = otx_base_hash(&otx, &layout, &raw, &[]).unwrap();

    let mut expected = [0u8; 32];
    let mut hasher = blake2b_ref::Blake2bBuilder::new(32)
        .personal(b"ckbcb_otb_core1\0")
        .build();
    hasher.update(&otx.message);
    hasher.update(&[otx.append_permissions]);
    hasher.update(&0u32.to_le_bytes());
    update_len_prefixed_for_test(&mut hasher, &[]);
    hasher.update(&0u32.to_le_bytes());
    update_len_prefixed_for_test(&mut hasher, &[]);
    hasher.update(&1u32.to_le_bytes());
    update_len_prefixed_for_test(&mut hasher, &[1]);
    hasher.update(&0u32.to_le_bytes());
    hasher.update(&[0x22]);
    hasher.update(&1u32.to_le_bytes());
    update_len_prefixed_for_test(&mut hasher, &[1]);
    hasher.update(&0u32.to_le_bytes());
    hasher.update(&[0x33; 32]);
    hasher.finalize(&mut expected);

    assert_eq!(actual, expected);
}

#[test]
fn otx_base_hash_streamed_cursor_fields_match_independent_preimage() {
    let previous_output = out_point_bytes([0x21; 32], 7);
    let input = cell_input_bytes(0x0102_0304_0506_0708, &previous_output);
    let lock = script_bytes([0x31; 32], 1, &[0x41, 0x42, 0x43]);
    let type_script = script_bytes([0x51; 32], 0, &[0x61, 0x62]);
    let output = cell_output_bytes(0x1112_1314_1516_1718, &lock, &type_script);
    CellInput::from(Cursor::new(input.len(), Box::new(OwnedReader::new(&input))))
        .verify(false)
        .unwrap();
    CellOutput::from(Cursor::new(
        output.len(),
        Box::new(OwnedReader::new(&output)),
    ))
    .verify(false)
    .unwrap();

    let resolved_output = vec![0x71, 0x72, 0x73];
    let resolved_data = vec![0x81, 0x82];

    let otx = OtxData {
        message: vec![0x91, 0x92],
        append_permissions: 0x03,
        base_input_cells: 1,
        base_input_masks: vec![0b0000_0011],
        base_output_cells: 1,
        base_output_masks: vec![0b0000_0110],
        base_cell_deps: 0,
        base_cell_dep_masks: Vec::new(),
        base_header_deps: 0,
        base_header_dep_masks: Vec::new(),
        append_input_cells: 0,
        append_output_cells: 0,
        append_cell_deps: 0,
        append_header_deps: 0,
        seals: Vec::new(),
    };
    let layout = OtxLayout {
        witness_index: 0,
        base_inputs: range(0, 1),
        append_inputs: range(1, 0),
        base_outputs: range(0, 1),
        append_outputs: range(1, 0),
        base_cell_deps: range(0, 0),
        append_cell_deps: range(0, 0),
        base_header_deps: range(0, 0),
        append_header_deps: range(0, 0),
    };
    let raw = RawTxParts {
        inputs: vec![input],
        outputs: vec![output],
        outputs_data: vec![vec![0xa1, 0xa2]],
        ..RawTxParts::default()
    };
    let resolved_inputs = vec![ResolvedInputHashPart {
        output: resolved_output.clone(),
        data: resolved_data.clone(),
    }];

    let actual = otx_base_hash(&otx, &layout, &raw, &resolved_inputs).unwrap();

    let mut expected = [0u8; 32];
    let mut hasher = blake2b_ref::Blake2bBuilder::new(32)
        .personal(b"ckbcb_otb_core1\0")
        .build();
    hasher.update(&otx.message);
    hasher.update(&[otx.append_permissions]);
    hasher.update(&1u32.to_le_bytes());
    update_len_prefixed_for_test(&mut hasher, &[0b0000_0011]);
    hasher.update(&0u32.to_le_bytes());
    hasher.update(&0x0102_0304_0506_0708u64.to_le_bytes());
    hasher.update(&previous_output);
    hasher.update(&resolved_output);
    update_len_prefixed_for_test(&mut hasher, &resolved_data);
    hasher.update(&1u32.to_le_bytes());
    update_len_prefixed_for_test(&mut hasher, &[0b0000_0110]);
    hasher.update(&0u32.to_le_bytes());
    hasher.update(&lock);
    hasher.update(&type_script);
    hasher.update(&0u32.to_le_bytes());
    update_len_prefixed_for_test(&mut hasher, &[]);
    hasher.update(&0u32.to_le_bytes());
    update_len_prefixed_for_test(&mut hasher, &[]);
    hasher.finalize(&mut expected);

    assert_eq!(actual, expected);
}

fn range(start: usize, count: usize) -> Range {
    Range { start, count }
}

fn out_point_bytes(tx_hash: [u8; 32], index: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(36);
    bytes.extend_from_slice(&tx_hash);
    bytes.extend_from_slice(&index.to_le_bytes());
    bytes
}

fn cell_input_bytes(since: u64, previous_output: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(44);
    bytes.extend_from_slice(&since.to_le_bytes());
    bytes.extend_from_slice(previous_output);
    bytes
}

fn script_bytes(code_hash: [u8; 32], hash_type: u8, args: &[u8]) -> Vec<u8> {
    table_bytes(&[code_hash.to_vec(), vec![hash_type], molecule_bytes(args)])
}

fn cell_output_bytes(capacity: u64, lock: &[u8], type_script: &[u8]) -> Vec<u8> {
    table_bytes(&[
        capacity.to_le_bytes().to_vec(),
        lock.to_vec(),
        type_script.to_vec(),
    ])
}

fn molecule_bytes(raw: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + raw.len());
    bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    bytes.extend_from_slice(raw);
    bytes
}

fn table_bytes(fields: &[Vec<u8>]) -> Vec<u8> {
    let header_size = 4 + fields.len() * 4;
    let total_size = header_size + fields.iter().map(Vec::len).sum::<usize>();
    let mut bytes = Vec::with_capacity(total_size);
    bytes.extend_from_slice(&(total_size as u32).to_le_bytes());

    let mut offset = header_size;
    for field in fields {
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += field.len();
    }
    for field in fields {
        bytes.extend_from_slice(field);
    }

    bytes
}

fn update_len_prefixed_for_test(hasher: &mut blake2b_ref::Blake2b, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u32).to_le_bytes());
    hasher.update(bytes);
}
