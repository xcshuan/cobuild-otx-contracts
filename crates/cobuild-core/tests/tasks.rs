use cobuild_core::{
    context::{CobuildContext, TxScriptHashes},
    error::CoreError,
    hash::{tx_with_message_hash, TxHashParts},
    layout::LayoutTx,
};

#[test]
fn lock_query_without_matching_lock_has_no_tasks() {
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
    let parts = TxHashParts {
        tx_hash: [0u8; 32],
        resolved_inputs: Vec::new(),
        trailing_witnesses: Vec::new(),
    };
    assert!(context
        .lock_query([2u8; 32])
        .tx_tasks(&parts)
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
    let parts = TxHashParts {
        tx_hash: [0u8; 32],
        resolved_inputs: Vec::new(),
        trailing_witnesses: Vec::new(),
    };

    assert!(context
        .lock_query([1u8; 32])
        .tx_tasks(&parts)
        .unwrap()
        .is_empty());

    let tasks = context.lock_query([2u8; 32]).tx_tasks(&parts).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].carrier_witness_index, 1);
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
    let parts = TxHashParts {
        tx_hash: [0u8; 32],
        resolved_inputs: Vec::new(),
        trailing_witnesses: Vec::new(),
    };

    assert!(matches!(
        context.lock_query([1u8; 32]).tx_tasks(&parts),
        Err(CoreError::MalformedCobuild | CoreError::InvalidLayout)
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
    let parts = TxHashParts {
        tx_hash: [0u8; 32],
        resolved_inputs: Vec::new(),
        trailing_witnesses: Vec::new(),
    };

    let tasks = context.lock_query([1u8; 32]).tx_tasks(&parts).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(
        tasks[0].signing_message_hash,
        tx_with_message_hash(&message, &parts).unwrap()
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
    let parts = TxHashParts {
        tx_hash: [0u8; 32],
        resolved_inputs: Vec::new(),
        trailing_witnesses: Vec::new(),
    };

    assert_eq!(
        context.lock_query([1u8; 32]).tx_tasks(&parts),
        Err(CoreError::DuplicateSealPair)
    );
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
