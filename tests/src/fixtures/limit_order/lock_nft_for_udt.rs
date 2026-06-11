use ckb_testtool::ckb_types::{bytes::Bytes, core::ScriptHashType, packed::Script, prelude::*};
use cobuild_types::entity::{
    core::{Message as CobuildMessage, Otx, SealPair, SighashAll},
    witness::{WitnessLayout, WitnessLayoutUnion},
};

use crate::fixtures::common::{
    assets::{nft_data, udt_amount_data},
    contracts::{
        deploy_always_success, deploy_input_type_proxy_lock, deploy_limit_order_lock,
        deploy_test_nft, deploy_test_udt, deploy_wrong_owner_lock,
    },
};
use crate::framework::{
    cells::{ResolvedInputFacts, TestCellOutput, live_resolved_facts, normal_output, typed_output},
    cobuild::{ActionRole, empty_message, seal_pair},
    contracts::{DeployedScript, cell_dep_for_script},
    fixture::CobuildTestFixture,
    scripts::script_hash,
    signing::{SignatureScope, SignerId, SigningFacts, SigningHashOracle, TestSigningHashOracle},
    tx::{BuiltTxShape, InputHandle, OtxHandle, OtxSegment, TxShape, WitnessHandle},
};

use super::{
    BuiltLimitOrderCase, BusinessMutation, CoverageTag, FlowKind, LimitOrderAction,
    LimitOrderExpectedOutcome, LimitOrderFixtureExt, LimitOrderHappyPath, LimitOrderLockError,
    LimitOrderState, NFT_TYPE_ARGS, OtxScopeKind, ScriptRoleKind, actions::encode_action,
    order_data,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LockFillCase {
    Valid,
    SighashAll,
    MalformedArgs,
    WrongNftType,
    TxLevelFillOrder,
    WrongActionTarget,
    OrderInputInAppendScope,
    InsufficientUdt,
    WrongUdt,
    WrongOwner,
    TxLevelRemainderOnly,
    PaymentInAnotherOtx,
    PaymentOutputOutOfRange,
    PaymentOutputWrongUdt,
    PaymentOutputWrongOwner,
    PaymentOutputInsufficient,
    MissingBuyerNftOutput,
    BuyerNftWrongLock,
    BuyerNftWrongType,
    UnknownActionTag,
    MalformedAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TwoLockOrdersCase {
    ReusePaymentOutput,
    DistinctPaymentOutputs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LockOrder {
    owner_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
    requested_asset_id: [u8; 32],
    requested_amount: u64,
}

pub fn lock_script_cases() -> Vec<BuiltLimitOrderCase> {
    let mut cases = Vec::new();
    cases.extend(lock_script_fill_cases());
    cases.extend(mixed_type_lock_cases());
    cases
}

pub fn lock_script_fill_cases() -> Vec<BuiltLimitOrderCase> {
    vec![
        lock_nft_for_udt_case(LockFillCase::Valid),
        lock_nft_for_udt_case(LockFillCase::SighashAll),
        lock_nft_for_udt_case(LockFillCase::MalformedArgs),
        lock_nft_for_udt_case(LockFillCase::WrongNftType),
        lock_nft_for_udt_case(LockFillCase::TxLevelFillOrder),
        lock_nft_for_udt_case(LockFillCase::WrongActionTarget),
        lock_nft_for_udt_case(LockFillCase::OrderInputInAppendScope),
        lock_nft_for_udt_case(LockFillCase::InsufficientUdt),
        lock_nft_for_udt_case(LockFillCase::WrongUdt),
        lock_nft_for_udt_case(LockFillCase::WrongOwner),
        lock_nft_for_udt_case(LockFillCase::TxLevelRemainderOnly),
        lock_nft_for_udt_case(LockFillCase::PaymentInAnotherOtx),
        lock_nft_for_udt_case(LockFillCase::PaymentOutputOutOfRange),
        lock_nft_for_udt_case(LockFillCase::PaymentOutputWrongUdt),
        lock_nft_for_udt_case(LockFillCase::PaymentOutputWrongOwner),
        lock_nft_for_udt_case(LockFillCase::PaymentOutputInsufficient),
        lock_nft_for_udt_case(LockFillCase::MissingBuyerNftOutput),
        lock_nft_for_udt_case(LockFillCase::BuyerNftWrongLock),
        lock_nft_for_udt_case(LockFillCase::BuyerNftWrongType),
        lock_nft_for_udt_case(LockFillCase::UnknownActionTag),
        lock_nft_for_udt_case(LockFillCase::MalformedAction),
        two_lock_orders_case(TwoLockOrdersCase::ReusePaymentOutput),
        two_lock_orders_case(TwoLockOrdersCase::DistinctPaymentOutputs),
    ]
}

pub fn mixed_type_lock_cases() -> Vec<BuiltLimitOrderCase> {
    vec![mixed_type_lock_duplicate_payment_case()]
}

fn lock_nft_for_udt_case(case: LockFillCase) -> BuiltLimitOrderCase {
    let mut fixture = CobuildTestFixture::new();
    let limit_order_lock_code = deploy_limit_order_lock(fixture.context_mut());
    let always_success = deploy_always_success(fixture.context_mut(), Vec::new());
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let wrong_owner = deploy_wrong_owner_lock(fixture.context_mut());
    let wrong_owner_lock = wrong_owner.script.clone();
    let wrong_buyer_lock = deploy_wrong_owner_lock(fixture.context_mut()).script;
    let nft = deploy_test_nft(fixture.context_mut(), NFT_TYPE_ARGS);
    let wrong_nft = deploy_test_nft(fixture.context_mut(), [6; 32]);
    let udt = deploy_test_udt(fixture.context_mut(), issuer_lock_hash);
    let wrong_udt = deploy_test_udt(fixture.context_mut(), [9; 32]);

    let order = LockOrder {
        owner_lock_hash: script_hash(&owner_lock),
        offered_nft_type_hash: nft.script_hash,
        requested_asset_id: udt.script_hash,
        requested_amount: 30,
    };
    let order_lock = lock_script(
        &mut fixture,
        &limit_order_lock_code,
        order,
        case == LockFillCase::MalformedArgs,
    );
    let order_lock_hash = script_hash(&order_lock);
    let seal_scope_value = if case == LockFillCase::OrderInputInAppendScope {
        1
    } else {
        0
    };

    let input_nft = if case == LockFillCase::WrongNftType {
        wrong_nft.clone()
    } else {
        nft.clone()
    };
    let payment_udt = if matches!(
        case,
        LockFillCase::WrongUdt | LockFillCase::PaymentOutputWrongUdt
    ) {
        wrong_udt.clone()
    } else {
        udt.clone()
    };
    let payment_lock = if matches!(
        case,
        LockFillCase::WrongOwner | LockFillCase::PaymentOutputWrongOwner
    ) {
        wrong_owner_lock
    } else {
        owner_lock.clone()
    };
    let insufficient_append_payment = matches!(
        case,
        LockFillCase::InsufficientUdt
            | LockFillCase::TxLevelRemainderOnly
            | LockFillCase::PaymentInAnotherOtx
            | LockFillCase::PaymentOutputInsufficient
    );
    let payment_amount = if insufficient_append_payment { 29 } else { 30 };

    let nft_payload = nft_data(b"lock-order-nft", [1, 2, 3, 4], 1_717_171_717);
    let nft_input = live_resolved_facts(
        fixture.context_mut(),
        typed_output(
            order_lock.clone(),
            input_nft.script.clone(),
            100_000_000_000,
        ),
        nft_payload.clone(),
    );
    let udt_input = live_resolved_facts(
        fixture.context_mut(),
        typed_output(
            buyer_lock.clone(),
            payment_udt.script.clone(),
            100_000_000_000,
        ),
        udt_amount_data(30),
    );
    let nft_output = match case {
        LockFillCase::MissingBuyerNftOutput => TestCellOutput::new(
            normal_output(always_success.script.clone(), 90_000_000_000),
            Vec::new(),
        ),
        LockFillCase::BuyerNftWrongLock => TestCellOutput::new(
            typed_output(wrong_buyer_lock, nft.script.clone(), 90_000_000_000),
            nft_payload.clone(),
        ),
        LockFillCase::BuyerNftWrongType => TestCellOutput::new(
            typed_output(buyer_lock.clone(), wrong_nft.script.clone(), 90_000_000_000),
            nft_payload.clone(),
        ),
        _ => TestCellOutput::new(
            typed_output(buyer_lock.clone(), input_nft.script.clone(), 90_000_000_000),
            nft_payload.clone(),
        ),
    };
    let udt_payment_output = TestCellOutput::new(
        typed_output(payment_lock, payment_udt.script.clone(), 90_000_000_000),
        udt_amount_data(payment_amount),
    );
    let dummy_base_input = matches!(
        case,
        LockFillCase::OrderInputInAppendScope | LockFillCase::PaymentInAnotherOtx
    )
    .then(|| {
        live_resolved_facts(
            fixture.context_mut(),
            normal_output(always_success.script.clone(), 100_000_000_000),
            Vec::new(),
        )
    });
    let remainder_payment_output = matches!(
        case,
        LockFillCase::TxLevelRemainderOnly | LockFillCase::PaymentOutputOutOfRange
    )
    .then(|| {
        TestCellOutput::new(
            typed_output(owner_lock.clone(), udt.script.clone(), 90_000_000_000),
            udt_amount_data(if case == LockFillCase::PaymentOutputOutOfRange {
                30
            } else {
                1
            }),
        )
    });
    let other_otx_payment_output = (case == LockFillCase::PaymentInAnotherOtx).then(|| {
        TestCellOutput::new(
            typed_output(owner_lock.clone(), udt.script.clone(), 90_000_000_000),
            udt_amount_data(1),
        )
    });

    let mut shape = TxShape::new();
    push_deps(
        &mut shape,
        [
            &limit_order_lock_code,
            &always_success,
            &wrong_owner,
            &nft,
            &wrong_nft,
            &udt,
            &wrong_udt,
        ],
    );
    let otx = if case == LockFillCase::OrderInputInAppendScope {
        shape.push_otx(OtxSegment {
            base_inputs: vec![dummy_base_input.clone().expect("dummy base input")],
            append_inputs: vec![nft_input, udt_input],
            base_outputs: vec![nft_output],
            append_outputs: vec![udt_payment_output],
            seals: vec![empty_seal_pair(order_lock_hash, seal_scope_value)],
            ..Default::default()
        })
    } else {
        shape.push_otx(OtxSegment {
            base_inputs: vec![nft_input],
            append_inputs: vec![udt_input],
            base_outputs: vec![nft_output],
            append_outputs: vec![udt_payment_output],
            seals: vec![empty_seal_pair(order_lock_hash, seal_scope_value)],
            ..Default::default()
        })
    };
    let seal_scope = if case == LockFillCase::OrderInputInAppendScope {
        SignatureScope::OtxAppend { otx }
    } else {
        SignatureScope::OtxBase { otx }
    };
    let order_input = if case == LockFillCase::OrderInputInAppendScope {
        shape.otx_append_input(otx, 0)
    } else {
        shape.otx_base_input(otx, 0)
    };
    let current_payment = shape.otx_append_output(otx, 0);
    let remainder_payment =
        remainder_payment_output.map(|output| shape.push_remainder_output(output));
    let other_payment = if let Some(output) = other_otx_payment_output {
        let other_otx = shape.push_otx(OtxSegment {
            base_inputs: vec![dummy_base_input.expect("dummy base input")],
            append_outputs: vec![output],
            ..Default::default()
        });
        Some(shape.otx_append_output(other_otx, 0))
    } else {
        None
    };
    if matches!(
        case,
        LockFillCase::SighashAll | LockFillCase::TxLevelFillOrder
    ) {
        shape.tx_level_message(empty_message());
    }

    let mut built = shape.build();
    let payment = match case {
        LockFillCase::PaymentInAnotherOtx => other_payment.expect("other OTX payment"),
        LockFillCase::PaymentOutputOutOfRange => remainder_payment.expect("remainder payment"),
        _ => current_payment,
    };
    let action = match case {
        LockFillCase::UnknownActionTag => LimitOrderAction::UnknownTag,
        LockFillCase::MalformedAction => LimitOrderAction::MalformedFill {
            payment,
            buyer_lock_hash: script_hash(&buyer_lock),
        },
        _ => LimitOrderAction::Fill {
            payment,
            buyer_lock_hash: script_hash(&buyer_lock),
        },
    };
    let target = if case == LockFillCase::WrongActionTarget {
        [8; 32]
    } else {
        order_lock_hash
    };
    let message = fill_message(&fixture, target, action, &built);
    if case == LockFillCase::TxLevelFillOrder {
        replace_tx_level_message(&mut built, message);
    } else {
        replace_otx_message(&mut built, otx, message);
    }

    let signing_facts = vec![empty_seal_facts(&built, order_lock_hash, seal_scope)];
    let expected = lock_fill_expected(case, order_input);

    built_case(
        format!("lock_fill::{case:?}"),
        fixture,
        built,
        signing_facts,
        expected,
        lock_fill_coverage(case),
    )
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
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![nft_input_a, nft_input_b],
        append_inputs: vec![udt_input],
        base_outputs: vec![nft_output_a, nft_output_b],
        append_outputs: vec![payment_output_a, payment_output_b],
        seals: vec![
            empty_seal_pair(order_lock_hash_a, 0),
            empty_seal_pair(order_lock_hash_b, 0),
        ],
        ..Default::default()
    });
    let base_scope = SignatureScope::OtxBase { otx };
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
            input_lock_error(order_b_input, LimitOrderLockError::InvalidAction)
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

fn mixed_type_lock_duplicate_payment_case() -> BuiltLimitOrderCase {
    let mut fixture = CobuildTestFixture::new();
    let limit_order_type = fixture.deploy_limit_order();
    let limit_order_lock_code = deploy_limit_order_lock(fixture.context_mut());
    let always_success = deploy_always_success(fixture.context_mut(), Vec::new());
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let proxy_lock =
        deploy_input_type_proxy_lock(fixture.context_mut(), limit_order_type.script_hash);
    let nft = deploy_test_nft(fixture.context_mut(), NFT_TYPE_ARGS);
    let lock_nft = deploy_test_nft(fixture.context_mut(), [0x73; 32]);
    let udt = deploy_test_udt(fixture.context_mut(), issuer_lock_hash);

    let type_order_input = limit_order_type_input(
        &mut fixture,
        owner_lock.clone(),
        nft.script_hash,
        udt.script_hash,
        30,
        &limit_order_type.script,
    );
    let lock_order = LockOrder {
        owner_lock_hash: script_hash(&owner_lock),
        offered_nft_type_hash: lock_nft.script_hash,
        requested_asset_id: udt.script_hash,
        requested_amount: 30,
    };
    let order_lock = lock_script(&mut fixture, &limit_order_lock_code, lock_order, false);
    let order_lock_hash = script_hash(&order_lock);

    let type_nft_payload = nft_data(b"mixed-type-order", [1, 2, 3, 4], 1_717_171_717);
    let lock_nft_payload = nft_data(b"mixed-lock-order", [5, 6, 7, 8], 1_717_171_718);
    let type_nft_input = live_resolved_facts(
        fixture.context_mut(),
        typed_output(
            proxy_lock.script.clone(),
            nft.script.clone(),
            100_000_000_000,
        ),
        type_nft_payload.clone(),
    );
    let lock_nft_input = live_resolved_facts(
        fixture.context_mut(),
        typed_output(order_lock, lock_nft.script.clone(), 100_000_000_000),
        lock_nft_payload.clone(),
    );
    let udt_input = live_resolved_facts(
        fixture.context_mut(),
        typed_output(buyer_lock.clone(), udt.script.clone(), 100_000_000_000),
        udt_amount_data(60),
    );
    let type_nft_output = TestCellOutput::new(
        typed_output(buyer_lock.clone(), nft.script.clone(), 90_000_000_000),
        type_nft_payload,
    );
    let lock_nft_output = TestCellOutput::new(
        typed_output(buyer_lock.clone(), lock_nft.script.clone(), 90_000_000_000),
        lock_nft_payload,
    );
    let payment_output = TestCellOutput::new(
        typed_output(owner_lock, udt.script.clone(), 90_000_000_000),
        udt_amount_data(30),
    );

    let mut shape = TxShape::new();
    push_deps(
        &mut shape,
        [
            &limit_order_type,
            &limit_order_lock_code,
            &always_success,
            &proxy_lock,
            &nft,
            &lock_nft,
            &udt,
        ],
    );
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![type_order_input, type_nft_input, lock_nft_input],
        append_inputs: vec![udt_input],
        base_outputs: vec![type_nft_output, lock_nft_output],
        append_outputs: vec![payment_output],
        seals: vec![empty_seal_pair(order_lock_hash, 0)],
        ..Default::default()
    });
    let base_scope = SignatureScope::OtxBase { otx };
    let lock_order_input = shape.otx_base_input(otx, 2);
    let shared_payment = shape.otx_append_output(otx, 0);
    let mut built = shape.build();
    let type_action = LimitOrderAction::Fill {
        payment: shared_payment,
        buyer_lock_hash: script_hash(&buyer_lock),
    };
    let lock_action = LimitOrderAction::Fill {
        payment: shared_payment,
        buyer_lock_hash: script_hash(&buyer_lock),
    };
    let message = fixture
        .cobuild()
        .push_action(
            ActionRole::InputType.into(),
            limit_order_type.script_hash,
            encode_action(&type_action, &built),
        )
        .push_action(
            ActionRole::InputLock.into(),
            order_lock_hash,
            encode_action(&lock_action, &built),
        )
        .build();
    replace_otx_message(&mut built, otx, message);

    built_case(
        "mixed_type_lock::DuplicatePaymentOutput",
        fixture,
        built.clone(),
        vec![empty_seal_facts(&built, order_lock_hash, base_scope)],
        input_lock_error(lock_order_input, LimitOrderLockError::InvalidAction),
        coverage(
            FlowKind::OtxOnly,
            ScriptRoleKind::InputLock,
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::Duplicate,
            Some(BusinessMutation::ReusePaymentOutput),
        ),
    )
}

fn lock_script(
    fixture: &mut CobuildTestFixture,
    limit_order_lock_code: &DeployedScript,
    order: LockOrder,
    malformed_args: bool,
) -> Script {
    let mut args = lock_args(order);
    if malformed_args {
        args.pop();
    }
    fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_lock_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&args),
        )
        .expect("build limit order lock")
}

fn limit_order_type_input(
    fixture: &mut CobuildTestFixture,
    owner: Script,
    offered_nft_type_hash: [u8; 32],
    requested_asset_id: [u8; 32],
    requested_amount: u64,
    limit_order_type: &Script,
) -> ResolvedInputFacts {
    let owner_lock_hash = script_hash(&owner);
    live_resolved_facts(
        fixture.context_mut(),
        typed_output(owner, limit_order_type.clone(), 100_000_000_000),
        order_data(LimitOrderState {
            owner_lock_hash,
            offered_nft_type_hash,
            requested_asset_id,
            requested_amount,
        }),
    )
}

fn fill_message(
    fixture: &CobuildTestFixture,
    target_hash: [u8; 32],
    action: LimitOrderAction,
    built: &BuiltTxShape,
) -> CobuildMessage {
    fixture
        .cobuild()
        .input_lock_action(target_hash)
        .action_data(encode_action(&action, built))
        .build()
}

fn replace_otx_message(built: &mut BuiltTxShape, otx: OtxHandle, message: CobuildMessage) {
    let witness = built.otx_witness(otx);
    let tx_index = built.witnesses.tx_index(witness);
    let current = built
        .tx
        .witnesses()
        .into_iter()
        .nth(tx_index)
        .expect("OTX witness")
        .raw_data();
    let otx = match WitnessLayout::from_slice(current.as_ref())
        .expect("parse OTX witness")
        .to_enum()
    {
        WitnessLayoutUnion::Otx(otx) => otx,
        other => panic!("expected OTX witness, got {}", other.item_name()),
    };
    let updated = otx.as_builder().message(message).build();
    replace_witness_bytes(built, tx_index, otx_witness_bytes(updated));
}

fn replace_tx_level_message(built: &mut BuiltTxShape, message: CobuildMessage) {
    let witness = WitnessLayout::from(
        SighashAll::new_builder()
            .seal(Vec::<u8>::new())
            .message(message)
            .build(),
    );
    replace_witness_bytes(built, 0, Bytes::copy_from_slice(witness.as_slice()));
}

fn replace_witness_bytes(built: &mut BuiltTxShape, tx_index: usize, replacement: Bytes) {
    let mut witnesses: Vec<_> = built.tx.witnesses().into_iter().collect();
    witnesses[tx_index] = replacement.pack();
    built.tx = built
        .tx
        .as_advanced_builder()
        .set_witnesses(witnesses)
        .build();
}

fn otx_witness_bytes(otx: Otx) -> Bytes {
    let witness = WitnessLayout::from(otx);
    Bytes::copy_from_slice(witness.as_slice())
}

fn empty_seal_pair(script_hash: [u8; 32], scope: u8) -> SealPair {
    seal_pair(script_hash, scope, Vec::new())
}

fn empty_seal_facts(
    built: &BuiltTxShape,
    script_hash: [u8; 32],
    scope: SignatureScope,
) -> SigningFacts {
    let oracle = TestSigningHashOracle;
    let signing_hash = match scope {
        SignatureScope::TxWithoutMessage | SignatureScope::TxWithMessage => {
            panic!("limit-order lock empty seals are OTX-scoped")
        }
        SignatureScope::OtxBase { otx } => oracle.otx_base(built, otx),
        SignatureScope::OtxAppend { otx } => {
            let base_hash = oracle.otx_base(built, otx);
            oracle.otx_append(built, otx, base_hash)
        }
    };
    SigningFacts {
        signer: SignerId("limit_order_lock_empty_seal"),
        scope,
        carrier: seal_carrier(built, scope),
        script_hash,
        signing_hash,
        seal: Vec::new(),
    }
}

fn seal_carrier(built: &BuiltTxShape, scope: SignatureScope) -> WitnessHandle {
    match scope {
        SignatureScope::OtxBase { otx } | SignatureScope::OtxAppend { otx } => {
            built.otx_witness(otx)
        }
        SignatureScope::TxWithoutMessage | SignatureScope::TxWithMessage => {
            panic!("limit-order lock empty seals are OTX-scoped")
        }
    }
}

fn lock_fill_expected(case: LockFillCase, input: InputHandle) -> LimitOrderExpectedOutcome {
    match case {
        LockFillCase::Valid | LockFillCase::SighashAll => LimitOrderExpectedOutcome::Pass,
        LockFillCase::MalformedArgs => input_lock_error(input, LimitOrderLockError::MalformedArgs),
        LockFillCase::WrongNftType => input_lock_error(input, LimitOrderLockError::WrongNftType),
        LockFillCase::UnknownActionTag => {
            input_lock_error(input, LimitOrderLockError::UnknownActionTag)
        }
        LockFillCase::MalformedAction => {
            input_lock_error(input, LimitOrderLockError::MalformedAction)
        }
        LockFillCase::InsufficientUdt
        | LockFillCase::WrongUdt
        | LockFillCase::WrongOwner
        | LockFillCase::TxLevelRemainderOnly
        | LockFillCase::PaymentOutputWrongUdt
        | LockFillCase::PaymentOutputWrongOwner
        | LockFillCase::PaymentOutputInsufficient => {
            input_lock_error(input, LimitOrderLockError::InvalidPayment)
        }
        LockFillCase::TxLevelFillOrder
        | LockFillCase::WrongActionTarget
        | LockFillCase::OrderInputInAppendScope
        | LockFillCase::PaymentInAnotherOtx
        | LockFillCase::PaymentOutputOutOfRange
        | LockFillCase::MissingBuyerNftOutput
        | LockFillCase::BuyerNftWrongLock
        | LockFillCase::BuyerNftWrongType => {
            input_lock_error(input, LimitOrderLockError::InvalidAction)
        }
    }
}

fn input_lock_error(input: InputHandle, error: LimitOrderLockError) -> LimitOrderExpectedOutcome {
    LimitOrderExpectedOutcome::InputLock { input, error }
}

fn built_case(
    name: impl Into<String>,
    fixture: CobuildTestFixture,
    built: BuiltTxShape,
    signing_facts: Vec<SigningFacts>,
    expected: LimitOrderExpectedOutcome,
    coverage: CoverageTag,
) -> BuiltLimitOrderCase {
    BuiltLimitOrderCase {
        name: name.into(),
        fixture,
        built,
        signing_facts,
        expected,
        coverage: vec![coverage],
    }
}

fn coverage(
    flow: FlowKind,
    script_role: ScriptRoleKind,
    otx_scope: OtxScopeKind,
    action_source: super::ActionSourceKind,
    mutation: Option<BusinessMutation>,
) -> CoverageTag {
    let tag = CoverageTag::new(flow, script_role, otx_scope, action_source);
    if let Some(mutation) = mutation {
        tag.with_mutation(mutation)
    } else {
        tag
    }
}

fn lock_fill_coverage(case: LockFillCase) -> CoverageTag {
    match case {
        LockFillCase::Valid => LimitOrderHappyPath::LockNftForUdt.default_coverage(),
        LockFillCase::SighashAll => coverage(
            FlowKind::TxLevelAndOtx,
            ScriptRoleKind::InputLock,
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::Otx,
            None,
        ),
        LockFillCase::TxLevelFillOrder => fill_mutation(
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::TxLevel,
            BusinessMutation::TxLevelActionInsteadOfOtxAction,
        ),
        LockFillCase::WrongActionTarget => fill_mutation(
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::WrongTarget,
            BusinessMutation::WrongActionTarget,
        ),
        LockFillCase::OrderInputInAppendScope => fill_mutation(
            OtxScopeKind::AppendInput,
            super::ActionSourceKind::Otx,
            BusinessMutation::OrderInputInAppendScope,
        ),
        LockFillCase::InsufficientUdt | LockFillCase::PaymentOutputInsufficient => fill_mutation(
            OtxScopeKind::AppendOutput,
            super::ActionSourceKind::Otx,
            BusinessMutation::PaymentOutputInsufficient,
        ),
        LockFillCase::WrongUdt | LockFillCase::PaymentOutputWrongUdt => fill_mutation(
            OtxScopeKind::AppendOutput,
            super::ActionSourceKind::Otx,
            BusinessMutation::PaymentOutputWrongUdt,
        ),
        LockFillCase::WrongOwner | LockFillCase::PaymentOutputWrongOwner => fill_mutation(
            OtxScopeKind::AppendOutput,
            super::ActionSourceKind::Otx,
            BusinessMutation::PaymentOutputWrongOwner,
        ),
        LockFillCase::TxLevelRemainderOnly | LockFillCase::PaymentOutputOutOfRange => {
            fill_mutation(
                OtxScopeKind::Remainder,
                super::ActionSourceKind::Otx,
                BusinessMutation::PaymentOutputInRemainder,
            )
        }
        LockFillCase::PaymentInAnotherOtx => fill_mutation(
            OtxScopeKind::AppendOutput,
            super::ActionSourceKind::Otx,
            BusinessMutation::PaymentOutputInAnotherOtx,
        ),
        LockFillCase::MissingBuyerNftOutput => fill_mutation(
            OtxScopeKind::BaseOutput,
            super::ActionSourceKind::Otx,
            BusinessMutation::BuyerNftMissing,
        ),
        LockFillCase::BuyerNftWrongLock => fill_mutation(
            OtxScopeKind::BaseOutput,
            super::ActionSourceKind::Otx,
            BusinessMutation::BuyerNftWrongLock,
        ),
        LockFillCase::BuyerNftWrongType => fill_mutation(
            OtxScopeKind::BaseOutput,
            super::ActionSourceKind::Otx,
            BusinessMutation::BuyerNftWrongType,
        ),
        LockFillCase::UnknownActionTag => fill_mutation(
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::Otx,
            BusinessMutation::UnknownActionTag,
        ),
        LockFillCase::MalformedAction => fill_mutation(
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::Otx,
            BusinessMutation::MalformedAction,
        ),
        LockFillCase::MalformedArgs | LockFillCase::WrongNftType => {
            LimitOrderHappyPath::LockNftForUdt.default_coverage()
        }
    }
}

fn fill_mutation(
    otx_scope: OtxScopeKind,
    action_source: super::ActionSourceKind,
    mutation: BusinessMutation,
) -> CoverageTag {
    coverage(
        FlowKind::OtxOnly,
        ScriptRoleKind::InputLock,
        otx_scope,
        action_source,
        Some(mutation),
    )
}

fn push_deps<'a>(shape: &mut TxShape, deps: impl IntoIterator<Item = &'a DeployedScript>) {
    for dep in deps {
        shape.push_prefix_cell_dep(cell_dep_for_script(dep));
    }
}

fn lock_args(order: LockOrder) -> Vec<u8> {
    let mut data = Vec::with_capacity(104);
    data.extend_from_slice(&order.owner_lock_hash);
    data.extend_from_slice(&order.offered_nft_type_hash);
    data.extend_from_slice(&order.requested_asset_id);
    data.extend_from_slice(&order.requested_amount.to_le_bytes());
    data
}
