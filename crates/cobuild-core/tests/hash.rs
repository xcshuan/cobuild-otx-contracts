use cobuild_core::{
    error::CoreError,
    hash::{
        checked_len_prefix, otx_base_hash, tx_without_message_hash, RawTxParts,
        ResolvedInputHashPart, TxHashParts,
    },
    layout::{OtxLayout, Range},
    view::OtxData,
};

#[test]
fn tx_without_message_hash_is_deterministic() {
    let parts = TxHashParts {
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
        Err(CoreError::MissingHashParts)
    );
}

#[test]
fn resolved_input_output_is_not_length_prefixed() {
    let parts = TxHashParts {
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

fn range(start: usize, count: usize) -> Range {
    Range { start, count }
}

fn update_len_prefixed_for_test(hasher: &mut blake2b_ref::Blake2b, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u32).to_le_bytes());
    hasher.update(bytes);
}
