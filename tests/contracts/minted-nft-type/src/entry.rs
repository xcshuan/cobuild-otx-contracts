use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    high_level::{QueryIter, load_cell_data, load_cell_type_hash, load_script},
};

use crate::{
    error::Error,
    types::{MinterState, nft_id, parse_minter_state, parse_nft_data},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NftMode {
    Create,
    Transfer,
    Burn,
}

pub fn nft_mode(input_count: usize, output_count: usize) -> Result<NftMode, Error> {
    match (input_count, output_count) {
        (0, 1) => Ok(NftMode::Create),
        (1, 1) => Ok(NftMode::Transfer),
        (1, 0) => Ok(NftMode::Burn),
        _ => Err(Error::InvalidShape),
    }
}

pub fn main() -> Result<(), Error> {
    validate_args_len()?;
    let input_count = QueryIter::new(load_cell_data, Source::GroupInput).count();
    let output_count = QueryIter::new(load_cell_data, Source::GroupOutput).count();
    match nft_mode(input_count, output_count)? {
        NftMode::Create => validate_create(),
        NftMode::Transfer => validate_transfer(),
        NftMode::Burn => Ok(()),
    }
}

fn validate_args_len() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    if args.len() != 32 {
        return Err(Error::InvalidArgs);
    }
    Ok(())
}

fn validate_create() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    let output = single_group_data(Source::GroupOutput)?;
    let nft = parse_nft_data(&output)?;
    let expected_id = nft_id(nft.minter_type_hash, nft.serial);
    if args.as_ref() != expected_id.as_slice() {
        return Err(Error::InvalidArgs);
    }
    let (input, output) = find_minter_transition(nft.minter_type_hash)?;
    serial_is_minted(input, output, nft.serial)
}

fn validate_transfer() -> Result<(), Error> {
    let input = single_group_data(Source::GroupInput)?;
    let output = single_group_data(Source::GroupOutput)?;
    parse_nft_data(&input)?;
    parse_nft_data(&output)?;
    if input != output {
        return Err(Error::InvalidNftData);
    }
    Ok(())
}

fn single_group_data(source: Source) -> Result<Bytes, Error> {
    let mut cells = QueryIter::new(load_cell_data, source);
    let Some(data) = cells.next() else {
        return Err(Error::InvalidNftData);
    };
    if cells.next().is_some() {
        return Err(Error::InvalidShape);
    }
    Ok(data.into())
}

fn find_minter_transition(minter_type_hash: [u8; 32]) -> Result<(MinterState, MinterState), Error> {
    let input = find_one_minter_state(Source::Input, minter_type_hash)?;
    let output = find_one_minter_state(Source::Output, minter_type_hash)?;
    Ok((input, output))
}

fn find_one_minter_state(source: Source, minter_type_hash: [u8; 32]) -> Result<MinterState, Error> {
    let mut found = None;
    for (index, type_hash) in QueryIter::new(load_cell_type_hash, source).enumerate() {
        if type_hash != Some(minter_type_hash) {
            continue;
        }
        if found.is_some() {
            return Err(Error::InvalidMinterTransition);
        }
        let data = load_cell_data(index, source)?;
        found = Some(parse_minter_state(&data)?);
    }
    found.ok_or(Error::InvalidMinterTransition)
}

pub fn serial_is_minted(input: MinterState, output: MinterState, serial: u64) -> Result<(), Error> {
    if input.supply_cap != output.supply_cap {
        return Err(Error::InvalidMinterTransition);
    }
    if output.mint_counter <= input.mint_counter {
        return Err(Error::InvalidMinterTransition);
    }
    if serial < input.mint_counter || serial >= output.mint_counter {
        return Err(Error::InvalidMinterTransition);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MinterState;

    #[test]
    fn nft_mode_detects_creation_transfer_and_burn() {
        assert_eq!(nft_mode(0, 1), Ok(NftMode::Create));
        assert_eq!(nft_mode(1, 1), Ok(NftMode::Transfer));
        assert_eq!(nft_mode(1, 0), Ok(NftMode::Burn));
        assert_eq!(nft_mode(0, 0), Err(Error::InvalidShape));
        assert_eq!(nft_mode(2, 1), Err(Error::InvalidShape));
    }

    #[test]
    fn serial_range_requires_counter_increment_covering_serial() {
        let input = MinterState {
            mint_counter: 6,
            supply_cap: 10,
        };
        let output = MinterState {
            mint_counter: 8,
            supply_cap: 10,
        };

        assert_eq!(serial_is_minted(input, output, 6), Ok(()));
        assert_eq!(serial_is_minted(input, output, 7), Ok(()));
        assert_eq!(
            serial_is_minted(input, output, 5),
            Err(Error::InvalidMinterTransition)
        );
        assert_eq!(
            serial_is_minted(input, output, 8),
            Err(Error::InvalidMinterTransition)
        );
        assert_eq!(
            serial_is_minted(
                input,
                MinterState {
                    mint_counter: 8,
                    supply_cap: 11,
                },
                6,
            ),
            Err(Error::InvalidMinterTransition)
        );
        assert_eq!(
            serial_is_minted(input, input, 6),
            Err(Error::InvalidMinterTransition)
        );
    }
}
