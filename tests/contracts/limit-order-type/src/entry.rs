#[cfg(not(feature = "type-id"))]
use ckb_std::high_level::load_script;
#[cfg(feature = "type-id")]
use ckb_std::type_id::check_type_id;
use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    high_level::{QueryIter, load_cell_data, load_script_hash},
};
use cobuild_core::{
    context::CurrentScript,
    engine::CobuildContext,
};

use crate::{
    error::Error,
    types::parse_order_state,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderMode {
    Create,
    Fill,
}

pub fn order_mode(input_count: usize, output_count: usize) -> Result<OrderMode, Error> {
    match (input_count, output_count) {
        (0, 1) => Ok(OrderMode::Create),
        (1, 0) => Ok(OrderMode::Fill),
        _ => Err(Error::InvalidOrderData),
    }
}

pub fn main() -> Result<(), Error> {
    let current_type_hash = load_script_hash()?;
    let context = CobuildContext::build(CurrentScript::Type(current_type_hash))?;
    let plan = context.plan_type_validation()?;

    let input_count = QueryIter::new(load_cell_data, Source::GroupInput).count();
    let output_count = QueryIter::new(load_cell_data, Source::GroupOutput).count();

    match order_mode(input_count, output_count)? {
        OrderMode::Create => crate::validation::validate_create_order(current_type_hash, &plan),
        OrderMode::Fill => crate::validation::validate_fill_order(&context, &plan),
    }
}

pub(crate) fn single_group_order(source: Source) -> Result<crate::types::OrderState, Error> {
    let mut cells = QueryIter::new(load_cell_data, source);
    let Some(data) = cells.next() else {
        return Err(Error::InvalidOrderData);
    };
    if cells.next().is_some() {
        return Err(Error::InvalidOrderData);
    }

    parse_order_state(&data)
}

#[cfg(feature = "type-id")]
pub(crate) fn validate_order_type_id() -> Result<(), Error> {
    check_type_id(0, 32).map_err(Error::from)
}

#[cfg(not(feature = "type-id"))]
pub(crate) fn validate_order_type_id() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    if args.len() < 32 {
        return Err(Error::TypeId);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_mode_accepts_create_shape() {
        assert_eq!(order_mode(0, 1), Ok(OrderMode::Create));
    }

    #[test]
    fn order_mode_accepts_fill_shape() {
        assert_eq!(order_mode(1, 0), Ok(OrderMode::Fill));
    }

    #[test]
    fn order_mode_rejects_update_or_empty_shapes() {
        assert_eq!(order_mode(1, 1), Err(Error::InvalidOrderData));
        assert_eq!(order_mode(0, 0), Err(Error::InvalidOrderData));
        assert_eq!(order_mode(2, 0), Err(Error::InvalidOrderData));
    }

    #[test]
    fn type_id_sys_error_maps_to_stable_exit_code() {
        #[cfg(feature = "type-id")]
        assert_eq!(
            Error::from(ckb_std::error::SysError::TypeIDError),
            Error::TypeId
        );
        assert_eq!(i8::from(Error::TypeId), 14);
    }
}
