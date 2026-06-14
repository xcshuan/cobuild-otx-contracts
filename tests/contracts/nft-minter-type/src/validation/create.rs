use ckb_std::ckb_constants::Source;
#[cfg(not(feature = "type-id"))]
use ckb_std::ckb_types::{bytes::Bytes, prelude::*};
#[cfg(not(feature = "type-id"))]
use ckb_std::high_level::load_script;
#[cfg(feature = "type-id")]
use ckb_std::type_id::check_type_id;
use cobuild_core::plan::TypeValidationPlan;

use crate::{
    error::Error,
    types::{MinterState, NftMinterAction},
    validation::helpers::{single_action, single_group_state},
};

pub fn validate_create(plan: &TypeValidationPlan) -> Result<(), Error> {
    validate_minter_type_id()?;
    let output = single_group_state(Source::GroupOutput)?;
    let action = single_action(plan)?;
    validate_create_state(output, action)
}

#[cfg(feature = "type-id")]
fn validate_minter_type_id() -> Result<(), Error> {
    check_type_id(0, 32).map_err(Error::from)
}

#[cfg(not(feature = "type-id"))]
fn validate_minter_type_id() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    if args.len() != 32 {
        return Err(Error::InvalidArgs);
    }
    Ok(())
}

pub fn validate_create_state(output: MinterState, action: NftMinterAction) -> Result<(), Error> {
    let NftMinterAction::CreateMinter { supply_cap } = action else {
        return Err(Error::InvalidAction);
    };
    if output.mint_counter != 0 {
        return Err(Error::Counter);
    }
    if output.supply_cap != supply_cap {
        return Err(Error::SupplyCap);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_requires_zero_counter_and_matching_cap() {
        let state = MinterState {
            mint_counter: 0,
            supply_cap: 10,
        };
        let action = NftMinterAction::CreateMinter { supply_cap: 10 };

        assert_eq!(validate_create_state(state, action), Ok(()));
    }

    #[test]
    fn create_rejects_non_zero_counter_or_cap_mismatch() {
        assert_eq!(
            validate_create_state(
                MinterState {
                    mint_counter: 1,
                    supply_cap: 10,
                },
                NftMinterAction::CreateMinter { supply_cap: 10 },
            ),
            Err(Error::Counter)
        );
        assert_eq!(
            validate_create_state(
                MinterState {
                    mint_counter: 0,
                    supply_cap: 9,
                },
                NftMinterAction::CreateMinter { supply_cap: 10 },
            ),
            Err(Error::SupplyCap)
        );
    }
}
