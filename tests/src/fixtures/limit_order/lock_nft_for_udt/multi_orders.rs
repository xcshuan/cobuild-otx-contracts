use super::*;
use crate::framework::scenario::{ExpectedOutcome, ScriptLocation};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TwoLockOrdersCase {
    ReusePaymentOutput,
    DistinctPaymentOutputs,
}

pub(super) fn multi_order_cases() -> Vec<BuiltLimitOrderCase> {
    vec![
        two_lock_orders_case(TwoLockOrdersCase::ReusePaymentOutput),
        two_lock_orders_case(TwoLockOrdersCase::DistinctPaymentOutputs),
    ]
}

fn two_lock_orders_case(case: TwoLockOrdersCase) -> BuiltLimitOrderCase {
    let mut fixture = CobuildTestFixture::new();
    let limit_order_lock_code = deploy_limit_order_lock(fixture.context_mut());
    let always_success = deploy_always_success(fixture.context_mut(), Vec::new());
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let nft_a = deploy_test_nft(fixture.context_mut(), [0x71; 32]);
    let nft_b = deploy_test_nft(fixture.context_mut(), [0x72; 32]);
    let udt = deploy_test_udt(fixture.context_mut(), issuer_lock_hash);

    let order_a = LockOrder {
        owner_lock_hash: script_hash(&owner_lock),
        offered_nft_type_hash: nft_a.script_hash,
        requested_asset_id: udt.script_hash,
        requested_amount: 30,
    };
    let order_b = LockOrder {
        offered_nft_type_hash: nft_b.script_hash,
        ..order_a
    };
    let order_lock_a = lock_script(&mut fixture, &limit_order_lock_code, order_a, false);
    let order_lock_b = lock_script(&mut fixture, &limit_order_lock_code, order_b, false);
    let order_lock_hash_a = script_hash(&order_lock_a);
    let order_lock_hash_b = script_hash(&order_lock_b);

    let nft_payload_a = nft_data(b"lock-order-a", [1, 2, 3, 4], 1_717_171_717);
    let nft_payload_b = nft_data(b"lock-order-b", [5, 6, 7, 8], 1_717_171_718);
    let nft_input_a = live_resolved_facts(
        fixture.context_mut(),
        typed_output(order_lock_a, nft_a.script.clone(), 100_000_000_000),
        nft_payload_a.clone(),
    );
    let nft_input_b = live_resolved_facts(
        fixture.context_mut(),
        typed_output(order_lock_b, nft_b.script.clone(), 100_000_000_000),
        nft_payload_b.clone(),
    );
    let udt_input = live_resolved_facts(
        fixture.context_mut(),
        typed_output(buyer_lock.clone(), udt.script.clone(), 100_000_000_000),
        udt_amount_data(60),
    );
    let nft_output_a = TestCellOutput::new(
        typed_output(buyer_lock.clone(), nft_a.script.clone(), 90_000_000_000),
        nft_payload_a,
    );
    let nft_output_b = TestCellOutput::new(
        typed_output(buyer_lock.clone(), nft_b.script.clone(), 90_000_000_000),
        nft_payload_b,
    );
    let payment_output_a = TestCellOutput::new(
        typed_output(owner_lock.clone(), udt.script.clone(), 90_000_000_000),
        udt_amount_data(30),
    );
    let payment_output_b = TestCellOutput::new(
        typed_output(owner_lock, udt.script.clone(), 90_000_000_000),
        udt_amount_data(30),
    );

    let mut shape = TxShape::new();
    push_deps(
        &mut shape,
        [
            &limit_order_lock_code,
            &always_success,
            &nft_a,
            &nft_b,
            &udt,
        ],
    );
    let otx = shape.push_otx(OtxSpec {
        base_inputs: vec![nft_input_a, nft_input_b],
        base_outputs: vec![nft_output_a, nft_output_b],
        append_segments: vec![
            append_segment_spec(0x00)
                .with_inputs(vec![udt_input])
                .with_outputs(vec![payment_output_a, payment_output_b]),
        ],
        base_seals: vec![
            empty_lock_seal(order_lock_hash_a),
            empty_lock_seal(order_lock_hash_b),
        ],
        ..Default::default()
    });
    let base_scope = SignatureScope::OtxBase { otx };
    let order_a_input = shape.otx_base_input(otx, 0);
    let order_b_input = shape.otx_base_input(otx, 1);
    let payment_a = shape.otx_append_output(otx, 0);
    let payment_b = shape.otx_append_output(otx, 1);
    let second_payment = if case == TwoLockOrdersCase::ReusePaymentOutput {
        payment_a
    } else {
        payment_b
    };
    let mut built = shape.build();
    let action_a = LimitOrderAction::Fill {
        payment: payment_a,
        buyer_lock_hash: script_hash(&buyer_lock),
    };
    let action_b = LimitOrderAction::Fill {
        payment: second_payment,
        buyer_lock_hash: script_hash(&buyer_lock),
    };
    let message = fixture
        .cobuild()
        .push_action(
            ActionRole::InputLock.into(),
            order_lock_hash_a,
            encode_action(&action_a, &built),
        )
        .push_action(
            ActionRole::InputLock.into(),
            order_lock_hash_b,
            encode_action(&action_b, &built),
        )
        .build();
    replace_otx_message(&mut built, otx, message);

    let signing_facts = vec![
        empty_seal_facts(&built, order_lock_hash_a, base_scope),
        empty_seal_facts(&built, order_lock_hash_b, base_scope),
    ];
    built_case(
        format!("two_lock_orders::{case:?}"),
        fixture,
        built,
        signing_facts,
        if case == TwoLockOrdersCase::ReusePaymentOutput {
            LimitOrderExpectedOutcome::Framework(ExpectedOutcome::AnyOf(vec![
                ExpectedOutcome::ScriptExit {
                    location: ScriptLocation::InputLock(order_a_input),
                    code: LimitOrderLockError::InvalidAction.code(),
                },
                ExpectedOutcome::ScriptExit {
                    location: ScriptLocation::InputLock(order_b_input),
                    code: LimitOrderLockError::InvalidAction.code(),
                },
            ]))
        } else {
            LimitOrderExpectedOutcome::Pass
        },
        coverage(
            FlowKind::OtxOnly,
            ScriptRoleKind::InputLock,
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::Duplicate,
            (case == TwoLockOrdersCase::ReusePaymentOutput)
                .then_some(BusinessMutation::ReusePaymentOutput),
        ),
    )
}
