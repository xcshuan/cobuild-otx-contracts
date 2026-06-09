use alloc::vec::Vec;

use cobuild_core::{
    engine::CobuildContext,
    plan::{ActionOrigin, OtxMessageLayout, OtxTypeRelation, TypeValidationPlan},
    protocol::ScriptRole,
    reader::cursor_bytes,
    view::ActionView,
};

use crate::{
    error::Error,
    types::{LimitOrderAction, parse_limit_order_action},
};

const FILL_ORDER_DATA_LEN: usize = 37;

pub struct TypeOtxFill {
    pub otx_index: usize,
    pub layout: OtxMessageLayout,
    pub action_data: Vec<u8>,
    pub action_target: [u8; 32],
}

pub fn load_type_otx_fill(
    context: &CobuildContext,
    plan: &TypeValidationPlan,
) -> Result<TypeOtxFill, Error> {
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
    Ok(TypeOtxFill {
        otx_index,
        layout,
        action_data: cursor_bytes(&related.action.action.data)?,
        action_target: related.action.action.script_hash,
    })
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
        plan::{ActionOrigin, OtxTypeRelation},
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
            otx_fill_layout(&origin, Some(relation(true)))
                .map(|(_, layout)| layout.append_outputs()),
            Ok(Range { start: 1, count: 1 })
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
