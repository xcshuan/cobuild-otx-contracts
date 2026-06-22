use ckb_testtool::ckb_types::{
    bytes::Bytes,
    prelude::{Builder, Entity},
};
use cobuild_types::entity::{core::SighashAllOnly, witness::WitnessLayout};
use k256::ecdsa::SigningKey;

pub type SecretKey = SigningKey;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SignerId(pub &'static str);

pub fn fixed_secret_key(byte: u8) -> SecretKey {
    SecretKey::from_slice(&[byte; 32]).expect("fixed secret key")
}

pub fn public_key_hash20(secret_key: &SecretKey) -> [u8; 20] {
    let public_key = secret_key.verifying_key().to_encoded_point(true);
    let hash = ckb_hash::blake2b_256(public_key.as_bytes());
    let mut out = [0u8; 20];
    out.copy_from_slice(&hash[..20]);
    out
}

pub fn sign_recoverable(secret_key: &SecretKey, digest: [u8; 32]) -> Vec<u8> {
    let (signature, recovery_id) = secret_key
        .sign_prehash_recoverable(&digest)
        .expect("recoverable signature");
    let mut seal = Vec::with_capacity(65);
    seal.extend_from_slice(&signature.to_bytes());
    seal.push(recovery_id.to_byte());
    seal
}

pub fn sighash_all_only_witness(seal: Vec<u8>) -> Bytes {
    let witness = WitnessLayout::from(SighashAllOnly::new_builder().seal(seal).build());
    Bytes::copy_from_slice(witness.as_slice())
}
