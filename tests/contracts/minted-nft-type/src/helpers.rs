use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    high_level::{QueryIter, load_cell_data, load_cell_type_hash, load_script},
};

use crate::{
    error::Error,
    types::{MinterState, parse_minter_state},
};

pub fn validate_args_len() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    if args.len() != 32 {
        return Err(Error::InvalidArgs);
    }
    Ok(())
}

pub fn single_group_data(source: Source) -> Result<Bytes, Error> {
    let mut cells = QueryIter::new(load_cell_data, source);
    let Some(data) = cells.next() else {
        return Err(Error::InvalidNftData);
    };
    if cells.next().is_some() {
        return Err(Error::InvalidShape);
    }
    Ok(data.into())
}

pub fn find_minter_transition(
    minter_type_hash: [u8; 32],
) -> Result<(MinterState, MinterState), Error> {
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
