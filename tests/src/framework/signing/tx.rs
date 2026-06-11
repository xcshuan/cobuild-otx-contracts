use blake2b_ref::{Blake2b, Blake2bBuilder};
use ckb_testtool::ckb_types::{core::TransactionView, prelude::Entity};
use cobuild_types::entity::core::Message as CobuildMessage;

use crate::framework::{scripts::packed_hash_to_array, tx::BuiltTxShape};

const TX_WITHOUT_MESSAGE_PERSONAL: &[u8; 16] = b"ckbcb_tnm_core1\0";
const TX_WITH_MESSAGE_PERSONAL: &[u8; 16] = b"ckbcb_twm_core1\0";

pub fn tx_without_message_hash(
    tx_hash: [u8; 32],
    input_count: usize,
    resolved_output: &[u8],
    witnesses: &[Vec<u8>],
) -> [u8; 32] {
    let inputs: Vec<(&[u8], &[u8])> = (0..input_count)
        .map(|_| (resolved_output, &[][..]))
        .collect();
    tx_without_message_hash_for_inputs(tx_hash, &inputs, witnesses)
}

pub fn tx_without_message_hash_for_inputs(
    tx_hash: [u8; 32],
    inputs: &[(&[u8], &[u8])],
    witnesses: &[Vec<u8>],
) -> [u8; 32] {
    tx_signing_hash(
        TX_WITHOUT_MESSAGE_PERSONAL,
        None,
        tx_hash,
        inputs,
        witnesses,
    )
}

pub fn tx_with_message_hash_for_inputs(
    tx_hash: [u8; 32],
    message: &CobuildMessage,
    inputs: &[(&[u8], &[u8])],
    witnesses: &[Vec<u8>],
) -> [u8; 32] {
    tx_signing_hash(
        TX_WITH_MESSAGE_PERSONAL,
        Some(message.as_slice()),
        tx_hash,
        inputs,
        witnesses,
    )
}

pub(crate) fn tx_without_message_hash_for_built(built: &BuiltTxShape) -> [u8; 32] {
    let inputs = resolved_input_bytes(built);
    let witnesses = witness_bytes(&built.tx);
    tx_without_message_hash_for_inputs(tx_hash(&built.tx), &inputs, &witnesses)
}

pub(crate) fn tx_with_message_hash_for_built(
    built: &BuiltTxShape,
    message: &CobuildMessage,
) -> [u8; 32] {
    let inputs = resolved_input_bytes(built);
    let witnesses = witness_bytes(&built.tx);
    tx_with_message_hash_for_inputs(tx_hash(&built.tx), message, &inputs, &witnesses)
}

pub fn checked_len_prefix(len: usize) -> [u8; 4] {
    u32::try_from(len)
        .expect("fixture length fits u32")
        .to_le_bytes()
}

fn tx_signing_hash(
    personalization: &[u8; 16],
    message: Option<&[u8]>,
    tx_hash: [u8; 32],
    inputs: &[(&[u8], &[u8])],
    witnesses: &[Vec<u8>],
) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut hasher = new_hasher(personalization);
    if let Some(message) = message {
        hasher.update(message);
    }
    hasher.update(&tx_hash);
    for (resolved_output, data) in inputs {
        hasher.update(resolved_output);
        hasher.update(&checked_len_prefix(data.len()));
        hasher.update(data);
    }
    for witness in witnesses.iter().skip(inputs.len()) {
        hasher.update(&checked_len_prefix(witness.len()));
        hasher.update(witness);
    }
    hasher.finalize(&mut out);
    out
}

pub(crate) fn resolved_input_bytes(built: &BuiltTxShape) -> Vec<(&[u8], &[u8])> {
    built
        .resolved_inputs
        .iter()
        .map(|input| (input.output.as_slice(), input.data.as_ref()))
        .collect()
}

pub(crate) fn witness_bytes(tx: &TransactionView) -> Vec<Vec<u8>> {
    tx.witnesses()
        .into_iter()
        .map(|witness| witness.raw_data().to_vec())
        .collect()
}

pub(crate) fn tx_hash(tx: &TransactionView) -> [u8; 32] {
    packed_hash_to_array(tx.hash())
}

pub(crate) fn new_hasher(personalization: &[u8; 16]) -> Blake2b {
    Blake2bBuilder::new(32).personal(personalization).build()
}
