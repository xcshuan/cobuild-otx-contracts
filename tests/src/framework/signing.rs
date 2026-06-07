use blake2b_ref::Blake2bBuilder;
use ckb_testtool::ckb_types::{
    bytes::Bytes,
    prelude::{Builder, Entity},
};
use cobuild_types::entity::{core::SighashAllOnly, witness::WitnessLayout};
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};

const TX_WITHOUT_MESSAGE_PERSONAL: &[u8; 16] = b"ckbcb_tnm_core1\0";

pub fn fixed_secret_key(byte: u8) -> SecretKey {
    SecretKey::from_slice(&[byte; 32]).expect("fixed secret key")
}

pub fn public_key_hash20(secret_key: &SecretKey) -> [u8; 20] {
    let secp = Secp256k1::new();
    let public_key = PublicKey::from_secret_key(&secp, secret_key);
    let hash = ckb_hash::blake2b_256(public_key.serialize());
    let mut out = [0u8; 20];
    out.copy_from_slice(&hash[..20]);
    out
}

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
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32)
        .personal(TX_WITHOUT_MESSAGE_PERSONAL)
        .build();
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

pub fn sign_recoverable(secret_key: &SecretKey, digest: [u8; 32]) -> Vec<u8> {
    let secp = Secp256k1::new();
    let message = Message::from_digest(digest);
    let signature = secp.sign_ecdsa_recoverable(&message, secret_key);
    let (recovery_id, compact) = signature.serialize_compact();
    let mut seal = Vec::with_capacity(65);
    seal.extend_from_slice(&compact);
    seal.push(i32::from(recovery_id) as u8);
    seal
}

pub fn sighash_all_only_witness(seal: Vec<u8>) -> Bytes {
    let witness = WitnessLayout::from(SighashAllOnly::new_builder().seal(seal).build());
    Bytes::copy_from_slice(witness.as_slice())
}

pub fn checked_len_prefix(len: usize) -> [u8; 4] {
    u32::try_from(len)
        .expect("fixture length fits u32")
        .to_le_bytes()
}
