use ckb_testtool::ckb_types::{
    bytes::Bytes,
    prelude::{Builder, Entity},
};
use cobuild_types::entity::{core::SighashAllOnly, witness::WitnessLayout};
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SignerId(pub &'static str);

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
