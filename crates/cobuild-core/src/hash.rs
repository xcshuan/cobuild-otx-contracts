use alloc::vec::Vec;

use blake2b_ref::Blake2bBuilder;

use crate::error::CoreError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TxHashParts {
    pub tx_hash: [u8; 32],
    pub resolved_inputs: Vec<ResolvedInputHashPart>,
    pub trailing_witnesses: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInputHashPart {
    pub output: Vec<u8>,
    pub data: Vec<u8>,
}

pub fn tx_without_message_hash(parts: &TxHashParts) -> Result<[u8; 32], CoreError> {
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32)
        .personal(b"ckbcb_tnm_core1\0")
        .build();

    hasher.update(&parts.tx_hash);
    for input in &parts.resolved_inputs {
        hasher.update(&input.output);
        update_len_prefixed(&mut hasher, &input.data)?;
    }
    for witness in &parts.trailing_witnesses {
        update_len_prefixed(&mut hasher, witness)?;
    }
    hasher.finalize(&mut out);

    Ok(out)
}

pub fn checked_len_prefix(len: usize) -> Result<[u8; 4], CoreError> {
    let len = u32::try_from(len).map_err(|_| CoreError::MissingHashParts)?;
    Ok(len.to_le_bytes())
}

fn update_len_prefixed(hasher: &mut blake2b_ref::Blake2b, bytes: &[u8]) -> Result<(), CoreError> {
    hasher.update(&checked_len_prefix(bytes.len())?);
    hasher.update(bytes);
    Ok(())
}
