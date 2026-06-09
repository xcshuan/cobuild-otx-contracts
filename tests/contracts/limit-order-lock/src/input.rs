use ckb_std::{
    ckb_constants::Source,
    high_level::{QueryIter, load_cell_data, load_cell_lock_hash, load_cell_type_hash},
};

use crate::error::Error;

pub fn load_current_order_input(
    current_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<usize, Error> {
    let input_index = single_group_input_index(current_lock_hash)?;
    verify_offered_nft_input(input_index, offered_nft_type_hash)?;
    Ok(input_index)
}

fn single_group_input_index(current_lock_hash: [u8; 32]) -> Result<usize, Error> {
    let group_count = QueryIter::new(load_cell_data, Source::GroupInput).count();
    if group_count != 1 {
        return Err(Error::InvalidNftInput);
    }

    let mut matching_inputs = QueryIter::new(load_cell_lock_hash, Source::Input)
        .enumerate()
        .filter_map(|(index, lock_hash)| (lock_hash == current_lock_hash).then_some(index));

    let Some(input_index) = matching_inputs.next() else {
        return Err(Error::InvalidNftInput);
    };
    if matching_inputs.next().is_some() {
        return Err(Error::InvalidNftInput);
    }

    Ok(input_index)
}

fn verify_offered_nft_input(
    input_index: usize,
    offered_nft_type_hash: [u8; 32],
) -> Result<(), Error> {
    let Some(type_hash) = load_cell_type_hash(input_index, Source::Input)? else {
        return Err(Error::InvalidNftInput);
    };
    if type_hash != offered_nft_type_hash {
        return Err(Error::InvalidNftInput);
    }
    Ok(())
}
