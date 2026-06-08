use alloc::vec::Vec;

use ckb_std::{
    ckb_constants::Source,
    ckb_types::prelude::Unpack,
    high_level::{QueryIter, load_cell_type_hash, load_script},
};

use crate::error::Error;

const TYPE_HASH_LEN: usize = 32;

pub fn main() -> Result<(), Error> {
    let script = load_script()?;
    let args: Vec<u8> = script.args().unpack();
    if args.len() < TYPE_HASH_LEN {
        return Err(Error::Encoding);
    }
    if has_matching_input_type(&args[..TYPE_HASH_LEN]) {
        return Ok(());
    }
    Err(Error::InvalidUnlock)
}

fn has_matching_input_type(owner_type_hash: &[u8]) -> bool {
    QueryIter::new(load_cell_type_hash, Source::Input).any(|cell_type_hash| {
        cell_type_hash
            .as_ref()
            .is_some_and(|cell_type_hash| owner_type_hash == cell_type_hash.as_slice())
    })
}

#[cfg(test)]
mod tests {
    use super::TYPE_HASH_LEN;

    #[test]
    fn type_hash_len_is_32_bytes() {
        assert_eq!(TYPE_HASH_LEN, 32);
    }
}
