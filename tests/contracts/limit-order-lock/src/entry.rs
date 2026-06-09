use alloc::vec::Vec;
use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    high_level::{
        QueryIter, load_cell_data, load_cell_lock_hash, load_cell_type_hash, load_script,
        load_script_hash,
    },
};
use cobuild_core::{
    context::CurrentScript,
    engine::CobuildContext,
    layout::Range,
    plan::{ActionOrigin, OtxMessageLayout},
    protocol::ScriptRole,
    reader::cursor_bytes,
    view::ActionView,
};

use crate::{
    error::Error,
    types::{
        UdtPayment, parse_fill_order_action, parse_order_args, parse_udt_payment, validate_fill,
    },
};

pub fn main() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    let order = parse_order_args(&args)?;

    let current_lock_hash = load_script_hash()?;
    let input_index = single_group_input_index(current_lock_hash)?;
    verify_offered_nft_input(input_index, order.offered_nft_type_hash)?;

    let context = CobuildContext::build(CurrentScript::InputLock(current_lock_hash))?;
    let plan = context.plan_lock_validation()?;
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }

    let related = &plan.related_actions[0];
    let (otx_index, layout) = otx_fill_layout(&related.origin, input_index)?;
    let actions = context.otx_actions(otx_index)?;
    let targets = limit_order_target_hashes(&actions, related.action.script_hash)?;
    ensure_unique_payment_output_indexes(&actions, &targets)?;
    let action_data = cursor_bytes(&related.action.data)?;
    let action = parse_fill_order_action(&action_data)?;
    let payment_output_index = action.payment_output_index as usize;
    if !output_index_in_otx_outputs(layout, payment_output_index)? {
        return Err(Error::InvalidCobuild);
    }
    let payment = load_udt_payment_output(payment_output_index)?;

    validate_fill(&order, payment)
}

fn single_group_input_index(current_lock_hash: [u8; 32]) -> Result<usize, Error> {
    let group_count = QueryIter::new(load_cell_data, Source::GroupInput).count();
    if group_count != 1 {
        return Err(Error::InvalidNftInput);
    }

    let mut matching_inputs = QueryIter::new(load_cell_lock_hash, Source::Input)
        .enumerate()
        .filter_map(|(index, lock_hash)| (lock_hash == current_lock_hash).then_some(index));

    let Some(input_index) = matching_inputs.next() else {
        return Err(Error::InvalidNftInput);
    };
    if matching_inputs.next().is_some() {
        return Err(Error::InvalidNftInput);
    }

    Ok(input_index)
}

fn verify_offered_nft_input(
    input_index: usize,
    offered_nft_type_hash: [u8; 32],
) -> Result<(), Error> {
    let Some(type_hash) = load_cell_type_hash(input_index, Source::Input)? else {
        return Err(Error::InvalidNftInput);
    };
    if type_hash != offered_nft_type_hash {
        return Err(Error::InvalidNftInput);
    }
    Ok(())
}

pub fn otx_fill_layout(
    origin: &ActionOrigin,
    input_index: usize,
) -> Result<(usize, OtxMessageLayout), Error> {
    let ActionOrigin::Otx {
        otx_index, layout, ..
    } = origin
    else {
        return Err(Error::InvalidCobuild);
    };
    if !range_contains(layout.base_inputs, input_index)? {
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

fn load_udt_payment_output(index: usize) -> Result<UdtPayment, Error> {
    let Some(asset_id) = load_cell_type_hash(index, Source::Output)? else {
        return Err(Error::InsufficientPayment);
    };
    let owner_lock_hash = load_cell_lock_hash(index, Source::Output)?;
    let data = load_cell_data(index, Source::Output)?;

    Ok(UdtPayment {
        owner_lock_hash,
        asset_id,
        amount: parse_udt_payment(&data)?,
    })
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
        let fill = parse_fill_order_action(&data)?;
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
        parse_fill_order_action(&data)?;
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

#[allow(dead_code)]
fn _source_marker(_: Source) {}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use super::*;
    use cobuild_core::reader::cursor_from_slice;

    fn layout() -> OtxMessageLayout {
        OtxMessageLayout {
            base_inputs: Range { start: 1, count: 1 },
            append_inputs: Range { start: 2, count: 1 },
            base_outputs: Range { start: 0, count: 1 },
            append_outputs: Range { start: 1, count: 1 },
            base_cell_deps: Range { start: 0, count: 0 },
            append_cell_deps: Range { start: 0, count: 0 },
            base_header_deps: Range { start: 0, count: 0 },
            append_header_deps: Range { start: 0, count: 0 },
        }
    }

    #[test]
    fn otx_fill_layout_accepts_current_input_in_base_scope() {
        let origin = ActionOrigin::Otx {
            witness_index: 0,
            otx_index: 0,
            layout: layout(),
        };

        assert_eq!(
            otx_fill_layout(&origin, 1).map(|(_, layout)| layout.append_outputs),
            Ok(Range { start: 1, count: 1 })
        );
    }

    #[test]
    fn otx_fill_layout_rejects_tx_level_action() {
        assert_eq!(
            otx_fill_layout(&ActionOrigin::TxLevel { witness_index: 0 }, 1),
            Err(Error::InvalidCobuild)
        );
    }

    #[test]
    fn otx_fill_layout_rejects_append_only_current_input() {
        let origin = ActionOrigin::Otx {
            witness_index: 0,
            otx_index: 0,
            layout: layout(),
        };

        assert_eq!(otx_fill_layout(&origin, 2), Err(Error::InvalidCobuild));
    }

    #[test]
    fn range_contains_accepts_start_and_last_index() {
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 3), Ok(true));
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 4), Ok(true));
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 5), Ok(false));
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
    fn range_contains_rejects_overflowing_range() {
        assert_eq!(
            range_contains(
                Range {
                    start: usize::MAX,
                    count: 1,
                },
                usize::MAX
            ),
            Err(Error::InvalidCobuild)
        );
    }

    #[test]
    fn duplicate_payment_output_index_accepts_unique_indexes() {
        let actions = vec![
            test_action(ScriptRole::InputLock, [7; 32], fill_data(1)),
            test_action(ScriptRole::InputLock, [7; 32], fill_data(2)),
        ];

        assert_eq!(
            ensure_unique_payment_output_indexes(&actions, &[[7; 32]]),
            Ok(())
        );
    }

    #[test]
    fn duplicate_payment_output_index_rejects_duplicate_indexes() {
        let actions = vec![
            test_action(ScriptRole::InputLock, [7; 32], fill_data(1)),
            test_action(ScriptRole::InputLock, [7; 32], fill_data(1)),
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
        let actions = vec![test_action(ScriptRole::InputType, [8; 32], malformed_fill)];

        assert_eq!(
            limit_order_target_hashes(&actions, [7; 32]),
            Err(Error::InvalidActionData)
        );
    }

    #[test]
    fn limit_order_target_hashes_ignores_unrelated_non_fill_actions() {
        let actions = vec![test_action(ScriptRole::InputType, [8; 32], vec![1, 2, 3])];

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
