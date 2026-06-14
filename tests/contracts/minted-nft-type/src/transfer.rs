use ckb_std::ckb_constants::Source;

use crate::{error::Error, helpers::single_group_data, types::parse_nft_data};

pub fn validate_transfer() -> Result<(), Error> {
    let input = single_group_data(Source::GroupInput)?;
    let output = single_group_data(Source::GroupOutput)?;
    parse_nft_data(&input)?;
    parse_nft_data(&output)?;
    if input != output {
        return Err(Error::InvalidNftData);
    }
    Ok(())
}
