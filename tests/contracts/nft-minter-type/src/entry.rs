use ckb_std::{
    ckb_constants::Source,
    high_level::{load_cell_data, load_script_hash, QueryIter},
};
use cobuild_core::{context::CurrentScript, engine::CobuildContext};

use crate::error::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MinterMode {
    Create,
    Mint,
    Burn,
}

pub fn minter_mode(input_count: usize, output_count: usize) -> Result<MinterMode, Error> {
    match (input_count, output_count) {
        (0, 1) => Ok(MinterMode::Create),
        (1, 1) => Ok(MinterMode::Mint),
        (1, 0) => Ok(MinterMode::Burn),
        _ => Err(Error::InvalidShape),
    }
}

pub fn main() -> Result<(), Error> {
    let current_type_hash = load_script_hash()?;
    let context = CobuildContext::build(CurrentScript::Type(current_type_hash))?;
    let plan = context.plan_type_validation()?;

    let input_count = QueryIter::new(load_cell_data, Source::GroupInput).count();
    let output_count = QueryIter::new(load_cell_data, Source::GroupOutput).count();

    match minter_mode(input_count, output_count)? {
        MinterMode::Create => crate::validation::validate_create(&plan),
        MinterMode::Mint => crate::validation::validate_mint(current_type_hash, &plan),
        MinterMode::Burn => Err(Error::InvalidShape),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minter_mode_detects_create_mint_and_burn() {
        assert_eq!(minter_mode(0, 1), Ok(MinterMode::Create));
        assert_eq!(minter_mode(1, 1), Ok(MinterMode::Mint));
        assert_eq!(minter_mode(1, 0), Ok(MinterMode::Burn));
        assert_eq!(minter_mode(0, 0), Err(Error::InvalidShape));
        assert_eq!(minter_mode(2, 1), Err(Error::InvalidShape));
    }
}
