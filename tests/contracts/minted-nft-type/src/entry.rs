use ckb_std::{
    ckb_constants::Source,
    high_level::{QueryIter, load_cell_data},
};

use crate::{error::Error, mode::NftMode};

pub fn main() -> Result<(), Error> {
    crate::helpers::validate_args_len()?;

    let input_count = QueryIter::new(load_cell_data, Source::GroupInput).count();
    let output_count = QueryIter::new(load_cell_data, Source::GroupOutput).count();

    match crate::mode::nft_mode(input_count, output_count)? {
        NftMode::Create => crate::create::validate_create(),
        NftMode::Transfer => crate::transfer::validate_transfer(),
        NftMode::Burn => Ok(()),
    }
}
