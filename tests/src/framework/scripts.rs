use ckb_testtool::ckb_types::{packed::Script, prelude::*};

pub fn script_hash(script: &Script) -> [u8; 32] {
    packed_hash_to_array(script.calc_script_hash())
}

pub fn packed_hash_to_array(hash: ckb_testtool::ckb_types::packed::Byte32) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(hash.as_slice());
    out
}
