use secp256k1::SecretKey;

use ckb_testtool::ckb_types::{
    bytes::Bytes,
    packed::Script,
    prelude::{Builder, Entity, Pack},
};

use crate::framework::{
    scripts::script_hash,
    signing::{SignerId, fixed_secret_key, public_key_hash20},
};

#[derive(Clone, Debug)]
pub struct Persona {
    pub id: SignerId,
    pub lock: Script,
    pub lock_hash: [u8; 32],
    pub secret_key: Option<SecretKey>,
    pub public_key_hash: [u8; 20],
}

impl Persona {
    pub fn fixed(name: &'static str, key_byte: u8) -> Self {
        let secret_key = fixed_secret_key(key_byte);
        let public_key_hash = public_key_hash20(&secret_key);
        let lock = persona_lock(name, public_key_hash);
        let lock_hash = script_hash(&lock);
        Self {
            id: SignerId(name),
            lock,
            lock_hash,
            secret_key: Some(secret_key),
            public_key_hash,
        }
    }

    pub fn unsigned(name: &'static str, lock: Script) -> Self {
        let lock_hash = script_hash(&lock);
        Self {
            id: SignerId(name),
            lock,
            lock_hash,
            secret_key: None,
            public_key_hash: [0u8; 20],
        }
    }
}

#[derive(Clone, Debug)]
pub struct Personas {
    pub owner: Persona,
    pub buyer: Persona,
    pub fee_payer: Persona,
    pub wrong_owner: Persona,
    pub order_lock_owner: Persona,
}

impl Default for Personas {
    fn default() -> Self {
        Self {
            owner: Persona::fixed("owner", 1),
            buyer: Persona::fixed("buyer", 2),
            fee_payer: Persona::fixed("fee_payer", 3),
            wrong_owner: Persona::fixed("wrong_owner", 4),
            order_lock_owner: Persona::fixed("order_lock_owner", 5),
        }
    }
}

fn persona_lock(name: &str, public_key_hash: [u8; 20]) -> Script {
    let mut args = Vec::with_capacity(name.len() + public_key_hash.len());
    args.extend_from_slice(name.as_bytes());
    args.extend_from_slice(&public_key_hash);
    Script::new_builder().args(Bytes::from(args).pack()).build()
}
