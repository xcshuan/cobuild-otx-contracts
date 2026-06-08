use alloc::vec::Vec;

#[cfg(not(feature = "type-id"))]
use ckb_std::high_level::load_script;
#[cfg(feature = "type-id")]
use ckb_std::type_id::check_type_id;
use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, packed::Script, prelude::*},
    high_level::{
        QueryIter, load_cell_data, load_cell_lock_hash, load_cell_type_hash, load_script_hash,
    },
};
use cobuild_core::{
    context::CurrentScript,
    engine::CobuildContext,
    layout::Range,
    plan::{ActionOrigin, OtxMessageLayout, OtxTypeRelation, TypeValidationPlan},
    reader::cursor_bytes,
};

use crate::{
    error::Error,
    types::{
        CreateOrderAction, LimitOrderAction, SETTLEMENT_DATA_LEN, SettlementCell,
        UDT_PAYMENT_DATA_LEN, parse_limit_order_action, parse_order_state, parse_settlement_cell,
        parse_udt_payment, validate_create, validate_fill,
    },
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
    let plan =
        CobuildContext::build(CurrentScript::Type(current_type_hash))?.plan_type_validation()?;

    let input_count = QueryIter::new(load_cell_data, Source::GroupInput).count();
    let output_count = QueryIter::new(load_cell_data, Source::GroupOutput).count();

    match order_mode(input_count, output_count)? {
        OrderMode::Create => validate_create_entry(current_type_hash, &plan),
        OrderMode::Fill => validate_fill_entry(&plan),
    }
}

fn validate_fill_entry(plan: &TypeValidationPlan) -> Result<(), Error> {
    let order = single_group_order(Source::GroupInput)?;
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }
    let related = &plan.related_actions[0];
    let layout = otx_fill_layout(
        &related.action.origin,
        related.otx_type_scope.in_otx_scope(),
    )?;
    let action_data = cursor_bytes(&related.action.action.data)?;
    let LimitOrderAction::Fill(action) = parse_limit_order_action(&action_data)? else {
        return Err(Error::UnsupportedAction);
    };
    let settlements = collect_settlements(layout)?;

    validate_fill(&order, &action, &settlements)
}

fn validate_create_entry(
    current_type_hash: [u8; 32],
    plan: &TypeValidationPlan,
) -> Result<(), Error> {
    validate_order_type_id()?;
    let order = single_group_order(Source::GroupOutput)?;
    let action = single_create_action(plan)?;
    validate_create(&order, &action)?;

    let proxy_lock_hash = expected_proxy_lock_hash(current_type_hash);
    if !has_nft_proxy_output(order.offered_nft_type_hash, proxy_lock_hash)? {
        return Err(Error::InvalidCobuild);
    }
    Ok(())
}

fn single_group_order(source: Source) -> Result<crate::types::OrderState, Error> {
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
fn validate_order_type_id() -> Result<(), Error> {
    check_type_id(0, 32).map_err(Error::from)
}

#[cfg(not(feature = "type-id"))]
fn validate_order_type_id() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    if args.len() < 32 {
        return Err(Error::TypeId);
    }
    Ok(())
}

fn expected_proxy_lock_hash(order_type_hash: [u8; 32]) -> [u8; 32] {
    let script = Script::new_builder()
        .code_hash(crate::generated_proxy_lock::INPUT_TYPE_PROXY_LOCK_CODE_HASH.pack())
        .hash_type(ckb_std::ckb_types::packed::Byte::new(4))
        .args(Bytes::copy_from_slice(&order_type_hash).pack())
        .build();
    script.calc_script_hash().unpack()
}

fn single_create_action(plan: &TypeValidationPlan) -> Result<CreateOrderAction, Error> {
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }
    let action_data = cursor_bytes(&plan.related_actions[0].action.action.data)?;
    let LimitOrderAction::Create(action) = parse_limit_order_action(&action_data)? else {
        return Err(Error::UnsupportedAction);
    };
    Ok(action)
}

fn has_nft_proxy_output(
    offered_nft_type_hash: [u8; 32],
    proxy_lock_hash: [u8; 32],
) -> Result<bool, Error> {
    let output_count = QueryIter::new(load_cell_data, Source::Output).count();
    for index in 0..output_count {
        let lock_hash = load_cell_lock_hash(index, Source::Output)?;
        if lock_hash != proxy_lock_hash {
            continue;
        }
        let Some(type_hash) = load_cell_type_hash(index, Source::Output)? else {
            continue;
        };
        if type_hash == offered_nft_type_hash {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn otx_fill_layout(
    origin: &ActionOrigin,
    relation: Option<OtxTypeRelation>,
) -> Result<OtxMessageLayout, Error> {
    let ActionOrigin::Otx { layout, .. } = origin else {
        return Err(Error::InvalidCobuild);
    };
    let Some(relation) = relation else {
        return Err(Error::InvalidCobuild);
    };
    if !relation.input_type_in_base {
        return Err(Error::InvalidCobuild);
    }

    Ok(*layout)
}

fn collect_settlements(layout: OtxMessageLayout) -> Result<Vec<SettlementCell>, Error> {
    let mut settlements = Vec::new();
    collect_settlements_from_range(layout.base_outputs, &mut settlements)?;
    collect_settlements_from_range(layout.append_outputs, &mut settlements)?;
    Ok(settlements)
}

fn collect_settlements_from_range(
    range: Range,
    settlements: &mut Vec<SettlementCell>,
) -> Result<(), Error> {
    let end = range
        .start
        .checked_add(range.count)
        .ok_or(Error::InvalidCobuild)?;

    for index in range.start..end {
        let data = load_cell_data(index, Source::Output)?;
        let lock_hash = load_cell_lock_hash(index, Source::Output)?;

        if data.len() == SETTLEMENT_DATA_LEN {
            settlements.push(parse_settlement_cell(lock_hash, &data)?);
            continue;
        }

        let Some(type_hash) = load_cell_type_hash(index, Source::Output)? else {
            continue;
        };
        if data.len() == UDT_PAYMENT_DATA_LEN {
            settlements.push(parse_udt_payment(lock_hash, type_hash, &data)?);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobuild_core::{
        layout::Range,
        plan::{ActionOrigin, OtxMessageLayout, OtxTypeRelation},
    };

    fn layout() -> OtxMessageLayout {
        OtxMessageLayout {
            base_inputs: Range { start: 0, count: 1 },
            append_inputs: Range { start: 1, count: 0 },
            base_outputs: Range { start: 0, count: 0 },
            append_outputs: Range { start: 0, count: 1 },
            base_cell_deps: Range { start: 0, count: 0 },
            append_cell_deps: Range { start: 0, count: 0 },
            base_header_deps: Range { start: 0, count: 0 },
            append_header_deps: Range { start: 0, count: 0 },
        }
    }

    fn relation(input_type_in_base: bool) -> OtxTypeRelation {
        OtxTypeRelation {
            input_type_in_base,
            input_type_in_append: false,
            output_type_in_base: false,
            output_type_in_base_covered: false,
            output_type_in_append: false,
        }
    }

    #[test]
    fn otx_fill_context_accepts_base_input_relation() {
        let origin = ActionOrigin::Otx {
            witness_index: 0,
            otx_index: 0,
            layout: layout(),
        };

        assert_eq!(
            otx_fill_layout(&origin, Some(relation(true))).map(|layout| layout.append_outputs),
            Ok(Range { start: 0, count: 1 })
        );
    }

    #[test]
    fn otx_fill_context_rejects_tx_level_action() {
        let origin = ActionOrigin::TxLevel { witness_index: 0 };

        assert_eq!(
            otx_fill_layout(&origin, None),
            Err(crate::error::Error::InvalidCobuild)
        );
    }

    #[test]
    fn otx_fill_context_rejects_non_base_input_relation() {
        let origin = ActionOrigin::Otx {
            witness_index: 0,
            otx_index: 0,
            layout: layout(),
        };

        assert_eq!(
            otx_fill_layout(&origin, Some(relation(false))),
            Err(crate::error::Error::InvalidCobuild)
        );
    }

    #[test]
    fn otx_fill_context_rejects_append_input_relation_only() {
        let origin = ActionOrigin::Otx {
            witness_index: 0,
            otx_index: 0,
            layout: layout(),
        };
        let mut relation = relation(false);
        relation.input_type_in_append = true;

        assert_eq!(
            otx_fill_layout(&origin, Some(relation)),
            Err(crate::error::Error::InvalidCobuild)
        );
    }

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

    #[test]
    fn expected_proxy_lock_hash_changes_with_order_type_hash() {
        let first = expected_proxy_lock_hash([1; 32]);
        let second = expected_proxy_lock_hash([2; 32]);

        assert_ne!(first, second);
    }

    #[test]
    fn create_action_context_accepts_any_origin_with_single_create_action() {
        let action = crate::types::LimitOrderAction::Create(crate::types::CreateOrderAction {
            owner_lock_hash: [2; 32],
            offered_nft_type_hash: [3; 32],
            requested_asset_id: [4; 32],
            min_requested_amount: 30,
        });

        assert!(matches!(action, crate::types::LimitOrderAction::Create(_)));
    }
}
