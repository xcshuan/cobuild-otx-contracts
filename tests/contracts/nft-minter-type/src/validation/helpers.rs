use ckb_std::{
    ckb_constants::Source,
    high_level::{QueryIter, load_cell_data},
};
use cobuild_core::{plan::TypeValidationPlan, reader::cursor_bytes};

use crate::{
    error::Error,
    types::{MinterState, NftMinterAction, parse_action, parse_minter_state},
};

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
