use cobuild_core::{
    layout::Range,
    plan::{
        LockValidationPlan, MessageOrigin, OtxMessageLayout, OtxTypeRelation, SignatureOrigin,
        SigningRequirement, TypeValidationPlan,
    },
};

#[test]
fn lock_validation_plan_carries_required_signatures() {
    let requirement = SigningRequirement {
        origin: SignatureOrigin::TxLevel,
        carrier_witness_index: 0,
        seal: vec![7u8; 65],
        signing_message_hash: [9u8; 32],
    };
    let plan = LockValidationPlan {
        lock_script_hash: [1u8; 32],
        required_signatures: vec![requirement.clone()],
    };

    assert_eq!(plan.lock_script_hash, [1u8; 32]);
    assert_eq!(plan.required_signatures, vec![requirement]);
}

#[test]
fn type_validation_plan_origin_carries_otx_layout_and_relation() {
    let origin = MessageOrigin::Otx {
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
        relation: OtxTypeRelation {
            input_type_in_base: true,
            input_type_in_append: false,
            output_type_in_base: true,
            output_type_in_base_covered: true,
            output_type_in_append: false,
        },
    };
    let plan = TypeValidationPlan {
        type_script_hash: [2u8; 32],
        related_messages: Vec::new(),
    };

    assert_eq!(plan.type_script_hash, [2u8; 32]);
    match origin {
        MessageOrigin::Otx {
            witness_index,
            otx_index,
            relation,
            ..
        } => {
            assert_eq!(witness_index, 4);
            assert_eq!(otx_index, 2);
            assert!(relation.input_type_in_base);
            assert!(relation.output_type_in_base);
            assert!(relation.output_type_in_base_covered);
        }
        MessageOrigin::TxLevel { .. } => panic!("expected otx origin"),
    }
}
