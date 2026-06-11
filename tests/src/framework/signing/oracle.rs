use cobuild_types::entity::core::Message as CobuildMessage;
use secp256k1::SecretKey;

use crate::framework::tx::{BuiltTxShape, OtxHandle, WitnessHandle};

use super::{
    keys::{SignerId, sign_recoverable},
    otx::{otx_append_hash, otx_base_hash},
    tx::{tx_with_message_hash_for_built, tx_without_message_hash_for_built},
};

pub trait SigningHashOracle {
    fn tx_without_message(&self, built: &BuiltTxShape) -> [u8; 32];
    fn tx_with_message(&self, built: &BuiltTxShape, message: &CobuildMessage) -> [u8; 32];
    fn otx_base(&self, built: &BuiltTxShape, otx: OtxHandle) -> [u8; 32];
    fn otx_append(&self, built: &BuiltTxShape, otx: OtxHandle, base_hash: [u8; 32]) -> [u8; 32];
}

pub struct TestSigningHashOracle;

impl SigningHashOracle for TestSigningHashOracle {
    fn tx_without_message(&self, built: &BuiltTxShape) -> [u8; 32] {
        tx_without_message_hash_for_built(built)
    }

    fn tx_with_message(&self, built: &BuiltTxShape, message: &CobuildMessage) -> [u8; 32] {
        tx_with_message_hash_for_built(built, message)
    }

    fn otx_base(&self, built: &BuiltTxShape, otx: OtxHandle) -> [u8; 32] {
        otx_base_hash(built, otx)
    }

    fn otx_append(&self, built: &BuiltTxShape, otx: OtxHandle, base_hash: [u8; 32]) -> [u8; 32] {
        otx_append_hash(built, otx, base_hash)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignatureScope {
    TxWithoutMessage,
    TxWithMessage,
    OtxBase { otx: OtxHandle },
    OtxAppend { otx: OtxHandle },
}

#[derive(Clone, Debug)]
pub struct SigningFacts {
    pub signer: SignerId,
    pub scope: SignatureScope,
    pub carrier: WitnessHandle,
    pub script_hash: [u8; 32],
    pub signing_hash: [u8; 32],
    pub seal: Vec<u8>,
}

pub fn assert_hash_changed(before: [u8; 32], after: [u8; 32]) {
    assert_ne!(before, after, "signing hash should change");
}

pub fn assert_hash_unchanged(before: [u8; 32], after: [u8; 32]) {
    assert_eq!(before, after, "signing hash should not change");
}

pub fn sign_scope(
    built: &BuiltTxShape,
    oracle: &impl SigningHashOracle,
    signer: SignerId,
    secret_key: &SecretKey,
    script_hash: [u8; 32],
    carrier: WitnessHandle,
    scope: SignatureScope,
) -> SigningFacts {
    let signing_hash = match scope {
        SignatureScope::TxWithoutMessage => oracle.tx_without_message(built),
        SignatureScope::TxWithMessage => {
            panic!("sign_scope requires a message parameter for TxWithMessage")
        }
        SignatureScope::OtxBase { otx } => oracle.otx_base(built, otx),
        SignatureScope::OtxAppend { otx } => {
            let base_hash = oracle.otx_base(built, otx);
            oracle.otx_append(built, otx, base_hash)
        }
    };
    let seal = sign_recoverable(secret_key, signing_hash);

    SigningFacts {
        signer,
        scope,
        carrier,
        script_hash,
        signing_hash,
        seal,
    }
}
