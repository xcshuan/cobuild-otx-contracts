use ckb_std::{
    ckb_constants::Source, ckb_types::bytes::Bytes, ckb_types::prelude::*, high_level::load_script,
};

use crate::{
    error::Error,
    helpers::{find_minter_transition, single_group_data},
    types::{MinterState, nft_id, parse_nft_data},
};

pub fn validate_create() -> Result<(), Error> {
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
