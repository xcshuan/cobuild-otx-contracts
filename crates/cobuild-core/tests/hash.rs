use cobuild_core::{
    error::CoreError,
    hash::{checked_len_prefix, tx_without_message_hash, ResolvedInputHashPart, TxHashParts},
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
