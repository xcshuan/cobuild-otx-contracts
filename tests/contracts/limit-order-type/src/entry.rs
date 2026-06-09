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
    protocol::ScriptRole,
    reader::cursor_bytes,
    view::ActionView,
};

use crate::{
    error::Error,
    types::{
        CreateOrderAction, LimitOrderAction, SettlementCell, parse_limit_order_action,
        parse_order_state, parse_udt_payment, validate_create, validate_fill,
    },
};

const FILL_ORDER_DATA_LEN: usize = 37;

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
        OrderMode::Create => validate_create_entry(current_type_hash, &plan),
        OrderMode::Fill => validate_fill_entry(&context, &plan),
    }
}

fn validate_fill_entry(context: &CobuildContext, plan: &TypeValidationPlan) -> Result<(), Error> {
    let order = single_group_order(Source::GroupInput)?;
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }
    let related = &plan.related_actions[0];
    let (otx_index, layout) = otx_fill_layout(
        &related.action.origin,
        related.otx_type_scope.in_otx_scope(),
    )?;
    let actions = context.otx_actions(otx_index)?;
    let targets = limit_order_target_hashes(&actions, related.action.action.script_hash)?;
    ensure_unique_payment_output_indexes(&actions, &targets)?;
    let action_data = cursor_bytes(&related.action.action.data)?;
    let LimitOrderAction::Fill(action) = parse_limit_order_action(&action_data)? else {
        return Err(Error::UnsupportedAction);
    };
    let payment_output_index = action.payment_output_index as usize;
    if !output_index_in_otx_outputs(layout, payment_output_index)? {
        return Err(Error::InvalidCobuild);
    }
    let payment = load_udt_payment_output(payment_output_index)?;

    validate_fill(&order, payment)
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
) -> Result<(usize, OtxMessageLayout), Error> {
    let ActionOrigin::Otx {
        otx_index, layout, ..
    } = origin
    else {
        return Err(Error::InvalidCobuild);
    };
    let Some(relation) = relation else {
        return Err(Error::InvalidCobuild);
    };
    if !relation.input_type_in_base {
        return Err(Error::InvalidCobuild);
    }

    Ok((*otx_index, *layout))
}

fn output_index_in_otx_outputs(
    layout: OtxMessageLayout,
    output_index: usize,
) -> Result<bool, Error> {
    Ok(range_contains(layout.base_outputs, output_index)?
        || range_contains(layout.append_outputs, output_index)?)
}

fn range_contains(range: Range, index: usize) -> Result<bool, Error> {
    let end = range
        .start
        .checked_add(range.count)
        .ok_or(Error::InvalidCobuild)?;
    Ok(index >= range.start && index < end)
}

fn load_udt_payment_output(index: usize) -> Result<SettlementCell, Error> {
    let data = load_cell_data(index, Source::Output)?;
    let lock_hash = load_cell_lock_hash(index, Source::Output)?;
    let Some(type_hash) = load_cell_type_hash(index, Source::Output)? else {
        return Err(Error::InsufficientPayment);
    };
    parse_udt_payment(lock_hash, type_hash, &data)
}

fn ensure_unique_payment_output_indexes(
    actions: &[ActionView],
    limit_order_targets: &[[u8; 32]],
) -> Result<(), Error> {
    let mut indexes = Vec::<u32>::new();
    for action in actions {
        if !is_limit_order_role(action.script_role) {
            continue;
        }
        if !limit_order_targets.contains(&action.script_hash) {
            continue;
        }
        let data = cursor_bytes(&action.data)?;
        if data.first().copied() != Some(crate::types::FILL_ORDER_TAG) {
            continue;
        }
        let LimitOrderAction::Fill(fill) = parse_limit_order_action(&data)? else {
            return Err(Error::InvalidCobuild);
        };
        if indexes.contains(&fill.payment_output_index) {
            return Err(Error::InvalidCobuild);
        }
        indexes.push(fill.payment_output_index);
    }
    Ok(())
}

fn limit_order_target_hashes(
    actions: &[ActionView],
    current_target: [u8; 32],
) -> Result<Vec<[u8; 32]>, Error> {
    let mut targets = Vec::<[u8; 32]>::new();
    targets.push(current_target);
    for action in actions {
        if !is_limit_order_role(action.script_role) {
            continue;
        }
        let data = cursor_bytes(&action.data)?;
        if data.first().copied() != Some(crate::types::FILL_ORDER_TAG) {
            continue;
        }
        if data.len() != FILL_ORDER_DATA_LEN {
            return Err(Error::InvalidActionData);
        }
        let LimitOrderAction::Fill(_) = parse_limit_order_action(&data)? else {
            return Err(Error::InvalidCobuild);
        };
        if !targets.contains(&action.script_hash) {
            targets.push(action.script_hash);
        }
    }
    Ok(targets)
}

fn is_limit_order_role(role: ScriptRole) -> bool {
    matches!(
        role,
        ScriptRole::InputType | ScriptRole::OutputType | ScriptRole::InputLock
    )
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use super::*;
    use cobuild_core::{
        layout::Range,
        plan::{ActionOrigin, OtxMessageLayout, OtxTypeRelation},
        reader::cursor_from_slice,
    };

    fn layout() -> OtxMessageLayout {
        OtxMessageLayout {
            base_inputs: Range { start: 0, count: 1 },
            append_inputs: Range { start: 1, count: 0 },
            base_outputs: Range { start: 0, count: 1 },
            append_outputs: Range { start: 1, count: 1 },
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
            otx_fill_layout(&origin, Some(relation(true))).map(|(_, layout)| layout.append_outputs),
            Ok(Range { start: 1, count: 1 })
        );
    }

    #[test]
    fn output_index_in_otx_outputs_accepts_base_and_append_outputs() {
        let layout = layout();

        assert_eq!(output_index_in_otx_outputs(layout, 0), Ok(true));
        assert_eq!(output_index_in_otx_outputs(layout, 1), Ok(true));
    }

    #[test]
    fn output_index_in_otx_outputs_rejects_out_of_range_output() {
        assert_eq!(output_index_in_otx_outputs(layout(), 2), Ok(false));
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
            requested_amount: 30,
        });

        assert!(matches!(action, crate::types::LimitOrderAction::Create(_)));
    }

    #[test]
    fn duplicate_payment_output_index_accepts_unique_indexes() {
        let actions = vec![
            test_action(ScriptRole::InputType, [7; 32], fill_data(1)),
            test_action(ScriptRole::InputType, [7; 32], fill_data(2)),
        ];

        assert_eq!(
            ensure_unique_payment_output_indexes(&actions, &[[7; 32]]),
            Ok(())
        );
    }

    #[test]
    fn duplicate_payment_output_index_rejects_duplicate_indexes() {
        let actions = vec![
            test_action(ScriptRole::InputType, [7; 32], fill_data(1)),
            test_action(ScriptRole::InputType, [7; 32], fill_data(1)),
        ];

        assert_eq!(
            ensure_unique_payment_output_indexes(&actions, &[[7; 32]]),
            Err(Error::InvalidCobuild)
        );
    }

    #[test]
    fn duplicate_payment_output_index_rejects_mixed_type_lock_duplicate() {
        let actions = vec![
            test_action(ScriptRole::InputType, [7; 32], fill_data(1)),
            test_action(ScriptRole::InputLock, [8; 32], fill_data(1)),
        ];

        assert_eq!(
            ensure_unique_payment_output_indexes(&actions, &[[7; 32], [8; 32]]),
            Err(Error::InvalidCobuild)
        );
    }

    #[test]
    fn limit_order_target_hashes_rejects_malformed_tag_two_in_selected_role() {
        let mut malformed_fill = fill_data(1);
        malformed_fill.pop();
        let actions = vec![test_action(ScriptRole::InputLock, [8; 32], malformed_fill)];

        assert_eq!(
            limit_order_target_hashes(&actions, [7; 32]),
            Err(Error::InvalidActionData)
        );
    }

    #[test]
    fn limit_order_target_hashes_ignores_unrelated_non_fill_actions() {
        let actions = vec![test_action(ScriptRole::InputLock, [8; 32], vec![1, 2, 3])];

        assert_eq!(
            limit_order_target_hashes(&actions, [7; 32]),
            Ok(vec![[7; 32]])
        );
    }

    fn fill_data(payment_output_index: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(crate::types::FILL_ORDER_TAG);
        data.extend_from_slice(&payment_output_index.to_le_bytes());
        data.extend_from_slice(&[9; 32]);
        data
    }

    fn test_action(script_role: ScriptRole, script_hash: [u8; 32], data: Vec<u8>) -> ActionView {
        ActionView {
            index: 0,
            script_info_hash: [0; 32],
            script_role,
            script_hash,
            data: cursor_from_slice(&data),
        }
    }
}
