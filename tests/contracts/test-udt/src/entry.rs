use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    high_level::{QueryIter, load_cell_data, load_cell_lock_hash, load_script},
};

use crate::error::Error;

const OWNER_LOCK_HASH_LEN: usize = 32;
const UDT_DATA_LEN: usize = 16;

pub fn main() -> Result<(), Error> {
    let script = load_script()?;
    let owner_lock_hash: Bytes = script.args().unpack();
    if owner_lock_hash.len() != OWNER_LOCK_HASH_LEN {
        return Err(Error::InvalidArgs);
    }

    if is_owner_mode(&owner_lock_hash) {
        return Ok(());
    }

    verify_amounts(
        collect_amounts(Source::GroupInput)?,
        collect_amounts(Source::GroupOutput)?,
    )
}

fn is_owner_mode(owner_lock_hash: &[u8]) -> bool {
    QueryIter::new(load_cell_lock_hash, Source::Input)
        .any(|lock_hash| owner_lock_hash == lock_hash.as_slice())
}

fn collect_amounts(source: Source) -> Result<u128, Error> {
    QueryIter::new(load_cell_data, source)
        .map(|data| parse_amount(&data))
        .try_fold(0u128, |total, amount| {
            total.checked_add(amount?).ok_or(Error::Amount)
        })
}

fn parse_amount(data: &[u8]) -> Result<u128, Error> {
    if data.len() != UDT_DATA_LEN {
        return Err(Error::Encoding);
    }

    let mut bytes = [0u8; UDT_DATA_LEN];
    bytes.copy_from_slice(data);
    Ok(u128::from_le_bytes(bytes))
}

fn verify_amounts(inputs_amount: u128, outputs_amount: u128) -> Result<(), Error> {
    if inputs_amount < outputs_amount {
        return Err(Error::Amount);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_amount_reads_16_byte_little_endian_u128() {
        let data = 42u128.to_le_bytes();

        assert_eq!(parse_amount(&data), Ok(42));
    }

    #[test]
    fn parse_amount_rejects_non_16_byte_data() {
        assert_eq!(parse_amount(&[0u8; 15]), Err(Error::Encoding));
        assert_eq!(parse_amount(&[0u8; 17]), Err(Error::Encoding));
    }

    #[test]
    fn amounts_are_conserved_when_outputs_do_not_exceed_inputs() {
        assert_eq!(verify_amounts(100, 100), Ok(()));
        assert_eq!(verify_amounts(100, 99), Ok(()));
    }

    #[test]
    fn amounts_fail_when_outputs_exceed_inputs() {
        assert_eq!(verify_amounts(100, 101), Err(Error::Amount));
    }
}
