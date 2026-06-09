use alloc::vec::Vec;

use cobuild_core::{
    engine::CobuildContext,
    layout::Range,
    plan::{ActionOrigin, OtxMessageLayout},
    protocol::ScriptRole,
    reader::cursor_bytes,
    view::ActionView,
};

use crate::{error::Error, types::parse_fill_order_action};

pub struct LockOtxFill {
    pub otx_index: usize,
    pub layout: OtxMessageLayout,
    pub action_data: Vec<u8>,
    pub action_target: [u8; 32],
}

pub fn load_lock_otx_fill(
    context: &CobuildContext,
    input_index: usize,
) -> Result<LockOtxFill, Error> {
    let plan = context.plan_lock_validation()?;
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }
    let related = &plan.related_actions[0];
    let (otx_index, layout) = otx_fill_layout(&related.origin, input_index)?;
    let actions = context.otx_actions(otx_index)?;
    let targets = limit_order_target_hashes(&actions, related.action.script_hash)?;
    ensure_unique_payment_output_indexes(&actions, &targets)?;
    Ok(LockOtxFill {
        otx_index,
        layout,
        action_data: cursor_bytes(&related.action.data)?,
        action_target: related.action.script_hash,
    })
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

pub fn output_index_in_otx_outputs(
    layout: OtxMessageLayout,
    output_index: usize,
) -> Result<bool, Error> {
    Ok(range_contains(layout.base_outputs, output_index)?
        || range_contains(layout.append_outputs, output_index)?)
}

pub fn ensure_unique_payment_output_indexes(
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

fn range_contains(range: Range, index: usize) -> Result<bool, Error> {
    let end = range
        .start
        .checked_add(range.count)
        .ok_or(Error::InvalidCobuild)?;
    Ok(index >= range.start && index < end)
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
