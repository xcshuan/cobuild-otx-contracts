use crate::error::Error;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nft_mode_detects_creation_transfer_and_burn() {
        assert_eq!(nft_mode(0, 1), Ok(NftMode::Create));
        assert_eq!(nft_mode(1, 1), Ok(NftMode::Transfer));
        assert_eq!(nft_mode(1, 0), Ok(NftMode::Burn));
        assert_eq!(nft_mode(0, 0), Err(Error::InvalidShape));
        assert_eq!(nft_mode(2, 1), Err(Error::InvalidShape));
    }
}
