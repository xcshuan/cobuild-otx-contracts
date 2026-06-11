use secp256k1::SecretKey;

use crate::framework::signing::{SignerId, fixed_secret_key, public_key_hash20};

#[derive(Clone, Debug)]
pub struct Persona {
    pub signer: SignerId,
    pub secret_key: SecretKey,
    pub public_key_hash: [u8; 20],
}

impl Persona {
    pub fn fixed(name: &'static str, key_byte: u8) -> Self {
        let secret_key = fixed_secret_key(key_byte);
        let public_key_hash = public_key_hash20(&secret_key);
        Self {
            signer: SignerId(name),
            secret_key,
            public_key_hash,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Personas {
    pub alice: Persona,
    pub bob: Persona,
    pub issuer: Persona,
    pub fee_payer: Persona,
}

impl Default for Personas {
    fn default() -> Self {
        Self {
            alice: Persona::fixed("alice", 1),
            bob: Persona::fixed("bob", 2),
            issuer: Persona::fixed("issuer", 3),
            fee_payer: Persona::fixed("fee_payer", 4),
        }
    }
}
