use ckb_std::{
    ckb_constants::Source,
    high_level::{QueryIter, load_cell_data},
};
use cobuild_core::{plan::TypeValidationPlan, reader::cursor_bytes};

use crate::{
    error::Error,
    types::{MinterState, NftMinterAction, parse_action, parse_minter_state},
};

pub fn validate_create(plan: &TypeValidationPlan) -> Result<(), Error> {
    crate::entry::validate_minter_type_id()?;
    let output = single_group_state(Source::GroupOutput)?;
    let action = single_action(plan)?;
    validate_create_state(output, action)
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

pub fn single_action(plan: &TypeValidationPlan) -> Result<NftMinterAction, Error> {
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }
    let action_data = cursor_bytes(&plan.related_actions[0].action.action.data)?;
    parse_action(&action_data)
}

pub fn single_group_state(source: Source) -> Result<MinterState, Error> {
    let mut cells = QueryIter::new(load_cell_data, source);
    let Some(data) = cells.next() else {
        return Err(Error::InvalidMinterData);
    };
    if cells.next().is_some() {
        return Err(Error::InvalidShape);
    }
    parse_minter_state(&data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MinterState, NftMinterAction};

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
