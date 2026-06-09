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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LimitOrderFillAction {
    payment_output_index: u32,
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
    ensure_no_reused_payment_outputs_in_otx(&actions)?;
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

pub fn resolve_otx_output_index(
    layout: OtxMessageLayout,
    relative_output_index: usize,
) -> Result<usize, Error> {
    // Fill actions name this field payment_output_index, but it is OTX-relative:
    // base outputs first, then append outputs.
    if relative_output_index < layout.base_outputs.count {
        return layout
            .base_outputs
            .start
            .checked_add(relative_output_index)
            .ok_or(Error::InvalidCobuild);
    }

    let append_index = relative_output_index - layout.base_outputs.count;
    if append_index < layout.append_outputs.count {
        return layout
            .append_outputs
            .start
            .checked_add(append_index)
            .ok_or(Error::InvalidCobuild);
    }

    Err(Error::InvalidCobuild)
}

fn ensure_no_reused_payment_outputs_in_otx(actions: &[ActionView]) -> Result<(), Error> {
    let fills = collect_limit_order_fill_actions(actions)?;
    let mut indexes = Vec::<u32>::new();
    for fill in fills {
        if indexes.contains(&fill.payment_output_index) {
            return Err(Error::InvalidCobuild);
        }
        indexes.push(fill.payment_output_index);
    }
    Ok(())
}

fn collect_limit_order_fill_actions(
    actions: &[ActionView],
) -> Result<Vec<LimitOrderFillAction>, Error> {
    let mut fills = Vec::<LimitOrderFillAction>::new();
    for action in actions {
        let Some(fill) = parse_limit_order_fill(action)? else {
            continue;
        };
        fills.push(fill);
    }
    Ok(fills)
}

fn parse_limit_order_fill(action: &ActionView) -> Result<Option<LimitOrderFillAction>, Error> {
    if !is_limit_order_role(action.script_role) {
        return Ok(None);
    }
    let data = cursor_bytes(&action.data)?;
    if data.first().copied() != Some(crate::types::FILL_ORDER_TAG) {
        return Ok(None);
    }
    let fill = parse_fill_order_action(&data)?;
    Ok(Some(LimitOrderFillAction {
        payment_output_index: fill.payment_output_index,
    }))
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
    fn resolve_otx_output_index_maps_relative_base_and_append_outputs() {
        let layout = OtxMessageLayout {
            base_outputs: Range { start: 4, count: 2 },
            append_outputs: Range { start: 9, count: 2 },
            ..layout()
        };

        assert_eq!(resolve_otx_output_index(layout, 0), Ok(4));
        assert_eq!(resolve_otx_output_index(layout, 1), Ok(5));
        assert_eq!(resolve_otx_output_index(layout, 2), Ok(9));
        assert_eq!(resolve_otx_output_index(layout, 3), Ok(10));
    }

    #[test]
    fn resolve_otx_output_index_rejects_out_of_range_relative_output() {
        assert_eq!(
            resolve_otx_output_index(layout(), 2),
            Err(Error::InvalidCobuild)
        );
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
    fn otx_payment_reuse_check_accepts_unique_indexes() {
        let actions = vec![
            test_action(ScriptRole::InputLock, [7; 32], fill_data(1)),
            test_action(ScriptRole::InputLock, [7; 32], fill_data(2)),
        ];

        assert_eq!(ensure_no_reused_payment_outputs_in_otx(&actions), Ok(()));
    }

    #[test]
    fn otx_payment_reuse_check_rejects_duplicate_indexes() {
        let actions = vec![
            test_action(ScriptRole::InputLock, [7; 32], fill_data(1)),
            test_action(ScriptRole::InputLock, [7; 32], fill_data(1)),
        ];

        assert_eq!(
            ensure_no_reused_payment_outputs_in_otx(&actions),
            Err(Error::InvalidCobuild)
        );
    }

    #[test]
    fn otx_payment_reuse_check_rejects_mixed_type_lock_duplicate() {
        let actions = vec![
            test_action(ScriptRole::InputType, [7; 32], fill_data(1)),
            test_action(ScriptRole::InputLock, [8; 32], fill_data(1)),
        ];

        assert_eq!(
            ensure_no_reused_payment_outputs_in_otx(&actions),
            Err(Error::InvalidCobuild)
        );
    }

    #[test]
    fn collect_limit_order_fill_actions_rejects_malformed_tag_two_in_selected_role() {
        let mut malformed_fill = fill_data(1);
        malformed_fill.pop();
        let actions = vec![test_action(ScriptRole::InputType, [8; 32], malformed_fill)];

        assert_eq!(
            collect_limit_order_fill_actions(&actions).map(|actions| actions.len()),
            Err(Error::InvalidActionData)
        );
    }

    #[test]
    fn collect_limit_order_fill_actions_ignores_unrelated_non_fill_actions() {
        let actions = vec![test_action(ScriptRole::InputType, [8; 32], vec![1, 2, 3])];

        assert_eq!(
            collect_limit_order_fill_actions(&actions).map(|actions| actions.len()),
            Ok(0)
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
