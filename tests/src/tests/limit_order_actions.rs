use super::*;
#[test]
fn limit_order_fill_action_encodes_payment_output_handle_tx_index() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, Bytes::new())],
        base_outputs: vec![signing_output(2, Bytes::new())],
        append_outputs: vec![
            signing_output(3, Bytes::new()),
            signing_output(4, Bytes::new()),
        ],
        ..Default::default()
    });
    let payment = shape.otx_append_output(otx, 1);
    let built = shape.build();

    let encoded = encode_action(
        &LimitOrderAction::Fill {
            payment,
            buyer_lock_hash: [0x42; 32],
        },
        &built,
    );

    assert_eq!(built.outputs.tx_index(payment), 2);
    assert_eq!(encoded[0], 2);
    assert_eq!(&encoded[1..5], &2u32.to_le_bytes());
    assert_eq!(&encoded[5..37], &[0x42; 32]);
}

#[test]
fn limit_order_create_action_encodes_order_state() {
    let built = TxShape::new().build();
    let order = LimitOrderState {
        owner_lock_hash: [1; 32],
        offered_nft_type_hash: [2; 32],
        requested_asset_id: [3; 32],
        requested_amount: 30,
    };

    let encoded = encode_action(&LimitOrderAction::Create { order }, &built);

    assert_eq!(encoded[0], 1);
    assert_eq!(&encoded[1..], order_data(order).as_ref());
}

#[test]
fn limit_order_unknown_action_uses_unknown_tag() {
    let built = TxShape::new().build();
    let encoded = encode_action(&LimitOrderAction::UnknownTag, &built);

    assert_ne!(encoded[0], 1);
    assert_ne!(encoded[0], 2);
}

#[test]
fn limit_order_duplicate_payment_is_expressed_by_reusing_the_same_output_handle() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, Bytes::new())],
        append_outputs: vec![signing_output(2, Bytes::new())],
        ..Default::default()
    });
    let shared_payment = shape.otx_append_output(otx, 0);
    let built = shape.build();

    let first = encode_action(
        &LimitOrderAction::Fill {
            payment: shared_payment,
            buyer_lock_hash: [0x11; 32],
        },
        &built,
    );
    let second = encode_action(
        &LimitOrderAction::Fill {
            payment: shared_payment,
            buyer_lock_hash: [0x22; 32],
        },
        &built,
    );

    assert_eq!(&first[1..5], &second[1..5]);
    assert_eq!(
        u32::from_le_bytes(first[1..5].try_into().expect("payment index")),
        built.outputs.tx_index(shared_payment) as u32
    );
}

#[test]
fn limit_order_payment_handles_can_point_outside_current_otx_output_range() {
    let mut shape = TxShape::new();
    let current_otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, Bytes::new())],
        append_outputs: vec![signing_output(2, Bytes::new())],
        ..Default::default()
    });
    let current_payment = shape.otx_append_output(current_otx, 0);
    let other_otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(3, Bytes::new())],
        append_outputs: vec![signing_output(4, Bytes::new())],
        ..Default::default()
    });
    let other_otx_payment = shape.otx_append_output(other_otx, 0);
    let remainder_payment = shape.push_remainder_output(signing_output(5, Bytes::new()));
    let built = shape.build();
    let current_range = &built.otx_ranges[0];

    assert!(
        current_range
            .append_outputs
            .contains(&built.outputs.tx_index(current_payment))
    );
    for payment in [other_otx_payment, remainder_payment] {
        let index = built.outputs.tx_index(payment);
        assert!(!current_range.base_outputs.contains(&index));
        assert!(!current_range.append_outputs.contains(&index));

        let encoded = encode_action(
            &LimitOrderAction::Fill {
                payment,
                buyer_lock_hash: [0x33; 32],
            },
            &built,
        );
        assert_eq!(
            u32::from_le_bytes(encoded[1..5].try_into().expect("payment index")),
            index as u32
        );
    }
}

#[test]
fn limit_order_error_mappings_match_contract_exit_codes() {
    assert_eq!(LimitOrderTypeError::InputAndOutputGroupShape.code(), 5);
    assert_eq!(LimitOrderTypeError::StateActionMismatch.code(), 10);
    assert_eq!(LimitOrderTypeError::InvalidPayment.code(), 11);
    assert_eq!(LimitOrderTypeError::InvalidAction.code(), 12);
    assert_eq!(LimitOrderTypeError::InvalidTypeId.code(), 14);

    assert_eq!(LimitOrderLockError::MalformedArgs.code(), 5);
    assert_eq!(LimitOrderLockError::MalformedAction.code(), 6);
    assert_eq!(LimitOrderLockError::UnknownActionTag.code(), 7);
    assert_eq!(LimitOrderLockError::WrongNftType.code(), 8);
    assert_eq!(LimitOrderLockError::InvalidPayment.code(), 10);
    assert_eq!(LimitOrderLockError::InvalidAction.code(), 12);
}

#[test]
fn limit_order_happy_path_coverage_has_full_tag_shape() {
    let tag = LimitOrderHappyPath::TwoTypeOrders
        .default_coverage()
        .with_mutation(BusinessMutation::ReusePaymentOutput);

    assert_eq!(tag.flow, FlowKind::OtxOnly);
    assert_eq!(tag.script_role, ScriptRoleKind::InputType);
    assert_eq!(tag.otx_scope, OtxScopeKind::BaseInput);
    assert_eq!(tag.action_source, ActionSourceKind::Duplicate);
    assert_eq!(tag.mutation, Some(BusinessMutation::ReusePaymentOutput));
}
