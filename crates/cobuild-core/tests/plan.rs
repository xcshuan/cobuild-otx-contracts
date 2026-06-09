use cobuild_core::{
    layout::{IndexRange, Range},
    plan::{
        ActionOrigin, LockValidationPlan, OtxMessageLayout, OtxTypeRelation, RelatedAction,
        SignatureOrigin, SigningRequirement, TypeActionOtxScope, TypeRelatedAction,
        TypeValidationPlan,
    },
    protocol::ScriptRole,
    reader::cursor_from_slice,
    view::ActionView,
};

#[test]
fn lock_validation_plan_carries_required_signatures_and_related_actions() {
    let requirement = SigningRequirement {
        origin: SignatureOrigin::TxLevel,
        carrier_witness_index: 0,
        seal: vec![7u8; 65],
        signing_message_hash: [9u8; 32],
    };
    let action = RelatedAction {
        origin: ActionOrigin::TxLevel { witness_index: 0 },
        action: ActionView {
            index: 0,
            script_info_hash: [3u8; 32],
            script_role: ScriptRole::InputLock,
            script_hash: [1u8; 32],
            data: cursor_from_slice(&[0x42]),
        },
    };
    let plan = LockValidationPlan {
        lock_script_hash: [1u8; 32],
        required_signatures: vec![requirement.clone()],
        related_actions: vec![action.clone()],
    };

    assert_eq!(plan.lock_script_hash, [1u8; 32]);
    assert_eq!(plan.required_signatures, vec![requirement]);
    assert_eq!(plan.related_actions.len(), 1);
    assert!(matches!(
        plan.related_actions[0].origin,
        ActionOrigin::TxLevel { witness_index: 0 }
    ));
    assert_eq!(plan.related_actions[0].action.data.size, 1);
}

#[test]
fn type_validation_plan_carries_type_specific_otx_relation() {
    let action = RelatedAction {
        origin: ActionOrigin::Otx {
            witness_index: 4,
            otx_index: 2,
            layout: OtxMessageLayout {
                base_inputs: Range { start: 1, count: 2 },
                append_inputs: Range { start: 3, count: 1 },
                base_outputs: Range { start: 0, count: 1 },
                append_outputs: Range { start: 1, count: 0 },
                base_cell_deps: Range { start: 0, count: 0 },
                append_cell_deps: Range { start: 0, count: 0 },
                base_header_deps: Range { start: 0, count: 0 },
                append_header_deps: Range { start: 0, count: 0 },
            },
        },
        action: ActionView {
            index: 0,
            script_info_hash: [3u8; 32],
            script_role: ScriptRole::InputType,
            script_hash: [2u8; 32],
            data: cursor_from_slice(&[0x24]),
        },
    };
    let related = TypeRelatedAction {
        action,
        otx_type_scope: TypeActionOtxScope::InOtxScope(OtxTypeRelation {
            input_type_in_base: true,
            input_type_in_append: false,
            output_type_in_base: true,
            output_type_in_base_covered: true,
            output_type_in_append: false,
        }),
    };
    let plan = TypeValidationPlan {
        type_script_hash: [2u8; 32],
        related_actions: vec![related],
    };

    assert_eq!(plan.type_script_hash, [2u8; 32]);
    assert!(
        plan.related_actions[0]
            .otx_type_scope
            .in_otx_scope()
            .unwrap()
            .input_type_in_base
    );
    match plan.related_actions[0].action.origin {
        ActionOrigin::Otx {
            witness_index,
            otx_index,
            layout,
            ..
        } => {
            assert_eq!(witness_index, 4);
            assert_eq!(otx_index, 2);
            assert_eq!(layout.base_inputs, Range { start: 1, count: 2 });
        }
        ActionOrigin::TxLevel { .. } => panic!("expected otx origin"),
    }
}

#[test]
fn otx_message_layout_exposes_combined_ranges_and_relative_indexes() {
    let layout = OtxMessageLayout {
        base_inputs: Range { start: 2, count: 2 },
        append_inputs: Range { start: 4, count: 1 },
        base_outputs: Range {
            start: 10,
            count: 2,
        },
        append_outputs: Range {
            start: 12,
            count: 2,
        },
        base_cell_deps: Range {
            start: 20,
            count: 1,
        },
        append_cell_deps: Range {
            start: 21,
            count: 2,
        },
        base_header_deps: Range {
            start: 30,
            count: 0,
        },
        append_header_deps: Range {
            start: 30,
            count: 1,
        },
    };

    assert_eq!(layout.inputs(), Range { start: 2, count: 3 });
    assert_eq!(layout.input_indexes(), IndexRange { start: 2, end: 5 });
    assert_eq!(
        layout.outputs(),
        Range {
            start: 10,
            count: 4
        }
    );
    assert_eq!(layout.output_indexes(), IndexRange { start: 10, end: 14 });
    assert_eq!(
        layout.output_indexes().into_iter().collect::<Vec<_>>(),
        vec![10, 11, 12, 13]
    );
    assert_eq!(
        layout.cell_deps(),
        Range {
            start: 20,
            count: 3
        }
    );
    assert_eq!(layout.cell_dep_indexes(), IndexRange { start: 20, end: 23 });
    assert_eq!(
        layout.header_deps(),
        Range {
            start: 30,
            count: 1
        }
    );
    assert_eq!(
        layout.header_dep_indexes(),
        IndexRange { start: 30, end: 31 }
    );

    assert_eq!(layout.base_inputs(), Range { start: 2, count: 2 });
    assert_eq!(
        layout.append_outputs(),
        Range {
            start: 12,
            count: 2
        }
    );
}

#[test]
fn type_validation_plan_names_target_only_actions_separately_from_otx_scope() {
    let action = RelatedAction {
        origin: ActionOrigin::Otx {
            witness_index: 1,
            otx_index: 0,
            layout: OtxMessageLayout {
                base_inputs: Range { start: 0, count: 1 },
                append_inputs: Range { start: 1, count: 0 },
                base_outputs: Range { start: 0, count: 0 },
                append_outputs: Range { start: 0, count: 0 },
                base_cell_deps: Range { start: 0, count: 0 },
                append_cell_deps: Range { start: 0, count: 0 },
                base_header_deps: Range { start: 0, count: 0 },
                append_header_deps: Range { start: 0, count: 0 },
            },
        },
        action: ActionView {
            index: 0,
            script_info_hash: [3u8; 32],
            script_role: ScriptRole::InputType,
            script_hash: [2u8; 32],
            data: cursor_from_slice(&[0x24]),
        },
    };
    let related = TypeRelatedAction {
        action,
        otx_type_scope: TypeActionOtxScope::TargetOnly,
    };

    assert!(related.otx_type_scope.in_otx_scope().is_none());
}
