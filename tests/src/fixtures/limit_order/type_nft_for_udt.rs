use ckb_hash::new_blake2b;
use ckb_testtool::ckb_types::{bytes::Bytes, core::ScriptHashType, packed::CellInput, prelude::*};
use cobuild_types::entity::{
    core::{Message as CobuildMessage, Otx, SighashAll},
    witness::{WitnessLayout, WitnessLayoutUnion},
};

use crate::fixtures::common::{
    assets::{nft_data, udt_amount_data},
    contracts::{
        deploy_always_success, deploy_input_type_proxy_lock, deploy_test_nft, deploy_test_udt,
        deploy_wrong_owner_lock,
    },
};
use crate::framework::{
    cells::{TestCellOutput, live_resolved_facts, normal_output, typed_output},
    cobuild::{ActionRole, empty_message},
    contracts::cell_dep_for_script,
    fixture::CobuildTestFixture,
    scripts::script_hash,
    tx::{BuiltTxShape, InputHandle, OtxHandle, OtxSegment, OutputHandle, TxShape},
};

use super::{
    BuiltLimitOrderCase, BusinessMutation, CoverageTag, FlowKind, LimitOrderAction,
    LimitOrderExpectedOutcome, LimitOrderFixtureExt, LimitOrderHappyPath, LimitOrderState,
    LimitOrderTypeError, NFT_TYPE_ARGS, OFFERED_ASSET_ID, OtxScopeKind, REQUESTED_ASSET_ID,
    ScriptRoleKind, actions::encode_action, order_data, settlement_data,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LegacySettlementCase {
    AtLimitPrice,
    BelowLimitPrice,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NftForUdtPaymentCase {
    Valid,
    InsufficientUdt,
    WrongUdt,
    WrongOwner,
    TxLevelRemainderOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FillActionCase {
    TxLevelFillOrder,
    OutputTypeTarget,
    PaymentInAnotherOtx,
    PaymentOutputOutOfRange,
    PaymentOutputWrongUdt,
    PaymentOutputWrongOwner,
    PaymentOutputInsufficient,
    MissingBuyerNftOutput,
    BuyerNftWrongLock,
    BuyerNftWrongType,
    TwoTypeOrdersReusePaymentOutput,
    TwoTypeOrdersUseDistinctPaymentOutputs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CreateOrderCase {
    Valid,
    MissingNftProxyOutput,
    WrongNftType,
    WrongProxyOrder,
    StateActionMismatch,
    InvalidTypeId,
    InputAndOutputGroupShape,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NftForUdtScenario {
    payment_case: NftForUdtPaymentCase,
    action_case: Option<FillActionCase>,
    sighash_all: bool,
}

impl NftForUdtScenario {
    fn payment(case: NftForUdtPaymentCase) -> Self {
        Self {
            payment_case: case,
            action_case: None,
            sighash_all: false,
        }
    }

    fn action(case: FillActionCase) -> Self {
        Self {
            payment_case: NftForUdtPaymentCase::Valid,
            action_case: Some(case),
            sighash_all: false,
        }
    }

    fn sighash_all() -> Self {
        Self {
            payment_case: NftForUdtPaymentCase::Valid,
            action_case: None,
            sighash_all: true,
        }
    }
}

pub fn type_script_cases() -> Vec<BuiltLimitOrderCase> {
    let mut cases = Vec::new();
    cases.extend(type_script_legacy_settlement_cases());
    cases.extend(type_script_fill_cases());
    cases.extend(type_script_create_order_cases());
    cases
}

pub fn type_script_legacy_settlement_cases() -> Vec<BuiltLimitOrderCase> {
    vec![
        legacy_settlement_case(LegacySettlementCase::AtLimitPrice),
        legacy_settlement_case(LegacySettlementCase::BelowLimitPrice),
    ]
}

pub fn type_script_fill_cases() -> Vec<BuiltLimitOrderCase> {
    vec![
        nft_for_udt_case(NftForUdtScenario::payment(NftForUdtPaymentCase::Valid)),
        nft_for_udt_case(NftForUdtScenario::sighash_all()),
        nft_for_udt_case(NftForUdtScenario::payment(
            NftForUdtPaymentCase::InsufficientUdt,
        )),
        nft_for_udt_case(NftForUdtScenario::payment(NftForUdtPaymentCase::WrongUdt)),
        nft_for_udt_case(NftForUdtScenario::payment(NftForUdtPaymentCase::WrongOwner)),
        nft_for_udt_case(NftForUdtScenario::payment(
            NftForUdtPaymentCase::TxLevelRemainderOnly,
        )),
        nft_for_udt_case(NftForUdtScenario::action(FillActionCase::TxLevelFillOrder)),
        nft_for_udt_case(NftForUdtScenario::action(FillActionCase::OutputTypeTarget)),
        nft_for_udt_case(NftForUdtScenario::action(
            FillActionCase::PaymentInAnotherOtx,
        )),
        nft_for_udt_case(NftForUdtScenario::action(
            FillActionCase::PaymentOutputOutOfRange,
        )),
        nft_for_udt_case(NftForUdtScenario::action(
            FillActionCase::PaymentOutputWrongUdt,
        )),
        nft_for_udt_case(NftForUdtScenario::action(
            FillActionCase::PaymentOutputWrongOwner,
        )),
        nft_for_udt_case(NftForUdtScenario::action(
            FillActionCase::PaymentOutputInsufficient,
        )),
        nft_for_udt_case(NftForUdtScenario::action(
            FillActionCase::MissingBuyerNftOutput,
        )),
        nft_for_udt_case(NftForUdtScenario::action(FillActionCase::BuyerNftWrongLock)),
        nft_for_udt_case(NftForUdtScenario::action(FillActionCase::BuyerNftWrongType)),
        two_type_orders_case(FillActionCase::TwoTypeOrdersReusePaymentOutput),
        two_type_orders_case(FillActionCase::TwoTypeOrdersUseDistinctPaymentOutputs),
    ]
}

pub fn type_script_create_order_cases() -> Vec<BuiltLimitOrderCase> {
    vec![
        create_order_case(CreateOrderCase::Valid),
        create_order_case(CreateOrderCase::MissingNftProxyOutput),
        create_order_case(CreateOrderCase::WrongNftType),
        create_order_case(CreateOrderCase::WrongProxyOrder),
        create_order_case(CreateOrderCase::StateActionMismatch),
        create_order_case(CreateOrderCase::InvalidTypeId),
        create_order_case(CreateOrderCase::InputAndOutputGroupShape),
    ]
}

fn legacy_settlement_case(case: LegacySettlementCase) -> BuiltLimitOrderCase {
    let mut fixture = CobuildTestFixture::new();
    let limit_order = fixture.deploy_limit_order();
    let always_success = deploy_always_success(fixture.context_mut(), Vec::new());
    let owner_lock = always_success.script.clone();
    let settlement_amount = match case {
        LegacySettlementCase::AtLimitPrice => 30,
        LegacySettlementCase::BelowLimitPrice => 29,
    };

    let order_input = limit_order_input(
        &mut fixture,
        owner_lock.clone(),
        OFFERED_ASSET_ID,
        REQUESTED_ASSET_ID,
        30,
        &limit_order.script,
    );
    let settlement_output = TestCellOutput::new(
        normal_output(owner_lock.clone(), 90_000_000_000),
        settlement_data(REQUESTED_ASSET_ID, settlement_amount),
    );

    let mut shape = TxShape::new();
    push_deps(&mut shape, [&limit_order, &always_success]);
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![order_input],
        append_outputs: vec![settlement_output],
        ..Default::default()
    });
    let order = shape.otx_base_input(otx, 0);
    let payment = shape.otx_append_output(otx, 0);
    let mut built = shape.build();
    let message = fill_message(
        &fixture,
        limit_order.script_hash,
        payment,
        script_hash(&owner_lock),
        &built,
    );
    replace_otx_message(&mut built, otx, message);

    built_case(
        fixture,
        built,
        input_type_error(order, LimitOrderTypeError::InvalidPayment),
        coverage(
            FlowKind::OtxOnly,
            ScriptRoleKind::InputType,
            OtxScopeKind::AppendOutput,
            super::ActionSourceKind::Otx,
            Some(match case {
                LegacySettlementCase::AtLimitPrice => BusinessMutation::PaymentOutputWrongUdt,
                LegacySettlementCase::BelowLimitPrice => {
                    BusinessMutation::PaymentOutputInsufficient
                }
            }),
        ),
    )
}

fn nft_for_udt_case(scenario: NftForUdtScenario) -> BuiltLimitOrderCase {
    let mut fixture = CobuildTestFixture::new();
    let limit_order = fixture.deploy_limit_order();
    let always_success = deploy_always_success(fixture.context_mut(), Vec::new());
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let wrong_owner_lock = deploy_wrong_owner_lock(fixture.context_mut()).script;
    let wrong_buyer_lock = deploy_wrong_owner_lock(fixture.context_mut()).script;
    let proxy_lock = deploy_input_type_proxy_lock(fixture.context_mut(), limit_order.script_hash);
    let nft = deploy_test_nft(fixture.context_mut(), NFT_TYPE_ARGS);
    let wrong_nft = deploy_test_nft(fixture.context_mut(), [0x66; 32]);
    let udt = deploy_test_udt(fixture.context_mut(), issuer_lock_hash);
    let wrong_udt = deploy_test_udt(fixture.context_mut(), [9; 32]);
    let payment_udt = if scenario.payment_case == NftForUdtPaymentCase::WrongUdt
        || scenario.action_case == Some(FillActionCase::PaymentOutputWrongUdt)
    {
        wrong_udt.clone()
    } else {
        udt.clone()
    };
    let payment_lock = if scenario.payment_case == NftForUdtPaymentCase::WrongOwner
        || scenario.action_case == Some(FillActionCase::PaymentOutputWrongOwner)
    {
        wrong_owner_lock
    } else {
        owner_lock.clone()
    };
    let insufficient_append_payment = matches!(
        scenario.payment_case,
        NftForUdtPaymentCase::InsufficientUdt | NftForUdtPaymentCase::TxLevelRemainderOnly
    ) || matches!(
        scenario.action_case,
        Some(FillActionCase::PaymentInAnotherOtx | FillActionCase::PaymentOutputInsufficient)
    );
    let payment_amount = if insufficient_append_payment { 29 } else { 30 };

    let nft_payload = nft_data(b"order-nft", [1, 2, 3, 4], 1_717_171_717);
    let order_input = limit_order_input(
        &mut fixture,
        owner_lock.clone(),
        nft.script_hash,
        udt.script_hash,
        30,
        &limit_order.script,
    );
    let nft_input = live_resolved_facts(
        fixture.context_mut(),
        typed_output(
            proxy_lock.script.clone(),
            nft.script.clone(),
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
    let nft_output = match scenario.action_case {
        Some(FillActionCase::MissingBuyerNftOutput) => TestCellOutput::new(
            normal_output(always_success.script.clone(), 90_000_000_000),
            Vec::new(),
        ),
        Some(FillActionCase::BuyerNftWrongLock) => TestCellOutput::new(
            typed_output(wrong_buyer_lock, nft.script.clone(), 90_000_000_000),
            nft_payload,
        ),
        Some(FillActionCase::BuyerNftWrongType) => TestCellOutput::new(
            typed_output(buyer_lock.clone(), wrong_nft.script.clone(), 90_000_000_000),
            nft_payload,
        ),
        _ => TestCellOutput::new(
            typed_output(buyer_lock.clone(), nft.script.clone(), 90_000_000_000),
            nft_payload,
        ),
    };
    let udt_payment_output = TestCellOutput::new(
        typed_output(payment_lock, payment_udt.script.clone(), 90_000_000_000),
        udt_amount_data(payment_amount),
    );
    let remainder_payment_output = if scenario.payment_case
        == NftForUdtPaymentCase::TxLevelRemainderOnly
        || scenario.action_case == Some(FillActionCase::PaymentOutputOutOfRange)
    {
        Some(TestCellOutput::new(
            typed_output(owner_lock.clone(), udt.script.clone(), 90_000_000_000),
            udt_amount_data(30),
        ))
    } else {
        None
    };
    let other_otx_payment_output =
        if scenario.action_case == Some(FillActionCase::PaymentInAnotherOtx) {
            Some(TestCellOutput::new(
                typed_output(owner_lock.clone(), udt.script.clone(), 90_000_000_000),
                udt_amount_data(1),
            ))
        } else {
            None
        };

    let mut shape = TxShape::new();
    push_deps(
        &mut shape,
        [
            &limit_order,
            &always_success,
            &proxy_lock,
            &nft,
            &wrong_nft,
            &udt,
            &wrong_udt,
        ],
    );
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![order_input, nft_input],
        append_inputs: vec![udt_input],
        base_outputs: vec![nft_output],
        append_outputs: vec![udt_payment_output],
        ..Default::default()
    });
    let order = shape.otx_base_input(otx, 0);
    let current_payment = shape.otx_append_output(otx, 0);
    let remainder_payment =
        remainder_payment_output.map(|output| shape.push_remainder_output(output));
    let other_payment = if let Some(output) = other_otx_payment_output {
        let dummy_input = live_resolved_facts(
            fixture.context_mut(),
            normal_output(always_success.script.clone(), 100_000_000_000),
            Vec::new(),
        );
        let other_otx = shape.push_otx(OtxSegment {
            base_inputs: vec![dummy_input],
            append_outputs: vec![output],
            ..Default::default()
        });
        Some(shape.otx_append_output(other_otx, 0))
    } else {
        None
    };
    if scenario.sighash_all || scenario.action_case == Some(FillActionCase::TxLevelFillOrder) {
        shape.tx_level_message(empty_message());
    }
    let mut built = shape.build();
    let payment = match scenario.action_case {
        Some(FillActionCase::PaymentInAnotherOtx) => other_payment.expect("other OTX payment"),
        Some(FillActionCase::PaymentOutputOutOfRange) => {
            remainder_payment.expect("remainder payment")
        }
        _ => current_payment,
    };
    let message = match scenario.action_case {
        Some(FillActionCase::OutputTypeTarget) => fill_output_type_message(
            &fixture,
            nft.script_hash,
            payment,
            script_hash(&buyer_lock),
            &built,
        ),
        _ => fill_message(
            &fixture,
            limit_order.script_hash,
            payment,
            script_hash(&buyer_lock),
            &built,
        ),
    };
    if scenario.action_case == Some(FillActionCase::TxLevelFillOrder) {
        replace_tx_level_message(&mut built, message);
    } else {
        replace_otx_message(&mut built, otx, message);
    }

    let expected = match scenario.payment_case {
        NftForUdtPaymentCase::Valid => match scenario.action_case {
            None => LimitOrderExpectedOutcome::Pass,
            Some(FillActionCase::PaymentOutputWrongUdt)
            | Some(FillActionCase::PaymentOutputWrongOwner)
            | Some(FillActionCase::PaymentOutputInsufficient) => {
                input_type_error(order, LimitOrderTypeError::InvalidPayment)
            }
            Some(_) => input_type_error(order, LimitOrderTypeError::InvalidAction),
        },
        _ => input_type_error(order, LimitOrderTypeError::InvalidPayment),
    };

    built_case(fixture, built, expected, fill_coverage(scenario))
}

fn two_type_orders_case(case: FillActionCase) -> BuiltLimitOrderCase {
    let mut fixture = CobuildTestFixture::new();
    let limit_order_code = fixture.deploy_limit_order();
    let always_success = deploy_always_success(fixture.context_mut(), Vec::new());
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let nft_a = deploy_test_nft(fixture.context_mut(), [0x51; 32]);
    let nft_b = deploy_test_nft(fixture.context_mut(), [0x52; 32]);
    let udt = deploy_test_udt(fixture.context_mut(), issuer_lock_hash);
    let order_type_a = fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&[0x61; 32]),
        )
        .expect("build first order type");
    let order_type_b = fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&[0x62; 32]),
        )
        .expect("build second order type");
    let order_type_hash_a = script_hash(&order_type_a);
    let order_type_hash_b = script_hash(&order_type_b);
    let proxy_lock_a = deploy_input_type_proxy_lock(fixture.context_mut(), order_type_hash_a);
    let proxy_lock_b = deploy_input_type_proxy_lock(fixture.context_mut(), order_type_hash_b);

    let order_input_a = limit_order_input(
        &mut fixture,
        owner_lock.clone(),
        nft_a.script_hash,
        udt.script_hash,
        30,
        &order_type_a,
    );
    let order_input_b = limit_order_input(
        &mut fixture,
        owner_lock.clone(),
        nft_b.script_hash,
        udt.script_hash,
        30,
        &order_type_b,
    );
    let nft_payload_a = nft_data(b"type-order-a", [1, 2, 3, 4], 1_717_171_717);
    let nft_payload_b = nft_data(b"type-order-b", [5, 6, 7, 8], 1_717_171_718);
    let nft_input_a = live_resolved_facts(
        fixture.context_mut(),
        typed_output(
            proxy_lock_a.script.clone(),
            nft_a.script.clone(),
            100_000_000_000,
        ),
        nft_payload_a.clone(),
    );
    let nft_input_b = live_resolved_facts(
        fixture.context_mut(),
        typed_output(
            proxy_lock_b.script.clone(),
            nft_b.script.clone(),
            100_000_000_000,
        ),
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
            &limit_order_code,
            &always_success,
            &proxy_lock_a,
            &proxy_lock_b,
            &nft_a,
            &nft_b,
            &udt,
        ],
    );
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![order_input_a, nft_input_a, order_input_b, nft_input_b],
        append_inputs: vec![udt_input],
        base_outputs: vec![nft_output_a, nft_output_b],
        append_outputs: vec![payment_output_a, payment_output_b],
        ..Default::default()
    });
    let order_b = shape.otx_base_input(otx, 2);
    let payment_a = shape.otx_append_output(otx, 0);
    let payment_b = shape.otx_append_output(otx, 1);
    let second_payment = if case == FillActionCase::TwoTypeOrdersReusePaymentOutput {
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
            ActionRole::InputType.into(),
            order_type_hash_a,
            encode_action(&action_a, &built),
        )
        .push_action(
            ActionRole::InputType.into(),
            order_type_hash_b,
            encode_action(&action_b, &built),
        )
        .build();
    replace_otx_message(&mut built, otx, message);

    built_case(
        fixture,
        built,
        if case == FillActionCase::TwoTypeOrdersReusePaymentOutput {
            input_type_error(order_b, LimitOrderTypeError::InvalidAction)
        } else {
            LimitOrderExpectedOutcome::Pass
        },
        coverage(
            FlowKind::OtxOnly,
            ScriptRoleKind::InputType,
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::Duplicate,
            (case == FillActionCase::TwoTypeOrdersReusePaymentOutput)
                .then_some(BusinessMutation::ReusePaymentOutput),
        ),
    )
}

fn create_order_case(case: CreateOrderCase) -> BuiltLimitOrderCase {
    let mut fixture = CobuildTestFixture::new();
    let limit_order_code = fixture.deploy_limit_order();
    let always_success = deploy_always_success(fixture.context_mut(), Vec::new());
    let owner_lock = always_success.script.clone();
    let funding_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(owner_lock.clone(), 200_000_000_000),
        Vec::new(),
    );
    let nft_type_id = type_id_args(&funding_input.input, 1);
    let nft = deploy_test_nft(fixture.context_mut(), nft_type_id);
    let output_nft = if case == CreateOrderCase::WrongNftType {
        deploy_test_nft(fixture.context_mut(), type_id_args(&funding_input.input, 2))
    } else {
        nft.clone()
    };
    let udt = deploy_test_udt(fixture.context_mut(), script_hash(&always_success.script));
    let computed_order_type_id = type_id_args(&funding_input.input, 0);
    let order_type_id = if case == CreateOrderCase::InvalidTypeId {
        [9; 32]
    } else {
        computed_order_type_id
    };
    let order_type = fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&order_type_id),
        )
        .expect("build order type-id script");
    let order_type_hash = script_hash(&order_type);
    let proxy_owner_type_hash = if case == CreateOrderCase::WrongProxyOrder {
        [8; 32]
    } else {
        order_type_hash
    };
    let proxy_lock = deploy_input_type_proxy_lock(fixture.context_mut(), proxy_owner_type_hash);
    let order_state = LimitOrderState {
        owner_lock_hash: script_hash(&owner_lock),
        offered_nft_type_hash: nft.script_hash,
        requested_asset_id: udt.script_hash,
        requested_amount: 30,
    };
    let action_state = LimitOrderState {
        requested_amount: if case == CreateOrderCase::StateActionMismatch {
            31
        } else {
            order_state.requested_amount
        },
        ..order_state
    };
    let order_output = TestCellOutput::new(
        typed_output(owner_lock.clone(), order_type.clone(), 100_000_000_000),
        order_data(order_state),
    );
    let wrong_nft_padding_output = if case == CreateOrderCase::WrongNftType {
        Some(TestCellOutput::new(
            normal_output(always_success.script.clone(), 10_000_000_000),
            Vec::new(),
        ))
    } else {
        None
    };
    let order_input = if case == CreateOrderCase::InputAndOutputGroupShape {
        Some(live_resolved_facts(
            fixture.context_mut(),
            typed_output(owner_lock, order_type.clone(), 100_000_000_000),
            order_data(order_state),
        ))
    } else {
        None
    };
    let nft_output = TestCellOutput::new(
        typed_output(
            proxy_lock.script.clone(),
            output_nft.script.clone(),
            90_000_000_000,
        ),
        nft_data(b"order-nft", [1, 2, 3, 4], 1_717_171_717),
    );

    let mut shape = TxShape::new();
    push_deps(
        &mut shape,
        [
            &limit_order_code,
            &always_success,
            &proxy_lock,
            &nft,
            &output_nft,
            &udt,
        ],
    );
    let order_input_handle = order_input.map(|input| shape.push_prefix_input(input));
    shape.push_prefix_input(funding_input);
    let order_output_handle = shape.push_remainder_output(order_output);
    if let Some(output) = wrong_nft_padding_output {
        shape.push_remainder_output(output);
    }
    if !matches!(
        case,
        CreateOrderCase::MissingNftProxyOutput | CreateOrderCase::InputAndOutputGroupShape
    ) {
        shape.push_remainder_output(nft_output);
    }
    shape.tx_level_message(empty_message());
    let mut built = shape.build();
    let action = LimitOrderAction::Create {
        order: action_state,
    };
    let message = fixture
        .cobuild()
        .output_type_action(order_type_hash)
        .action_data(encode_action(&action, &built))
        .build();
    replace_tx_level_message(&mut built, message);

    let expected = match case {
        CreateOrderCase::Valid => LimitOrderExpectedOutcome::Pass,
        CreateOrderCase::StateActionMismatch => output_type_error(
            order_output_handle,
            LimitOrderTypeError::StateActionMismatch,
        ),
        CreateOrderCase::InvalidTypeId => {
            output_type_error(order_output_handle, LimitOrderTypeError::InvalidTypeId)
        }
        CreateOrderCase::InputAndOutputGroupShape => input_type_error(
            order_input_handle.expect("order input handle"),
            LimitOrderTypeError::InputAndOutputGroupShape,
        ),
        _ => output_type_error(order_output_handle, LimitOrderTypeError::InvalidAction),
    };

    built_case(fixture, built, expected, create_coverage(case))
}

fn limit_order_input(
    fixture: &mut CobuildTestFixture,
    owner: ckb_testtool::ckb_types::packed::Script,
    offered_nft_type_hash: [u8; 32],
    requested_asset_id: [u8; 32],
    requested_amount: u64,
    limit_order_type: &ckb_testtool::ckb_types::packed::Script,
) -> crate::framework::cells::ResolvedInputFacts {
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
    payment: OutputHandle,
    buyer_lock_hash: [u8; 32],
    built: &BuiltTxShape,
) -> CobuildMessage {
    let action = LimitOrderAction::Fill {
        payment,
        buyer_lock_hash,
    };
    fixture
        .cobuild()
        .input_type_action(target_hash)
        .action_data(encode_action(&action, built))
        .build()
}

fn fill_output_type_message(
    fixture: &CobuildTestFixture,
    target_hash: [u8; 32],
    payment: OutputHandle,
    buyer_lock_hash: [u8; 32],
    built: &BuiltTxShape,
) -> CobuildMessage {
    let action = LimitOrderAction::Fill {
        payment,
        buyer_lock_hash,
    };
    fixture
        .cobuild()
        .output_type_action(target_hash)
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

fn input_type_error(input: InputHandle, error: LimitOrderTypeError) -> LimitOrderExpectedOutcome {
    LimitOrderExpectedOutcome::InputType { input, error }
}

fn output_type_error(
    output: OutputHandle,
    error: LimitOrderTypeError,
) -> LimitOrderExpectedOutcome {
    LimitOrderExpectedOutcome::OutputType { output, error }
}

fn built_case(
    fixture: CobuildTestFixture,
    built: BuiltTxShape,
    expected: LimitOrderExpectedOutcome,
    coverage: CoverageTag,
) -> BuiltLimitOrderCase {
    BuiltLimitOrderCase {
        fixture,
        built,
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

fn fill_coverage(scenario: NftForUdtScenario) -> CoverageTag {
    if scenario.sighash_all {
        return coverage(
            FlowKind::TxLevelAndOtx,
            ScriptRoleKind::InputType,
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::Otx,
            None,
        );
    }
    match (scenario.payment_case, scenario.action_case) {
        (NftForUdtPaymentCase::Valid, None) => {
            LimitOrderHappyPath::TypeNftForUdt.default_coverage()
        }
        (NftForUdtPaymentCase::InsufficientUdt, _) => {
            fill_mutation(BusinessMutation::PaymentOutputInsufficient)
        }
        (NftForUdtPaymentCase::WrongUdt, _) => {
            fill_mutation(BusinessMutation::PaymentOutputWrongUdt)
        }
        (NftForUdtPaymentCase::WrongOwner, _) => {
            fill_mutation(BusinessMutation::PaymentOutputWrongOwner)
        }
        (NftForUdtPaymentCase::TxLevelRemainderOnly, _) => coverage(
            FlowKind::TxLevelAndOtx,
            ScriptRoleKind::InputType,
            OtxScopeKind::Remainder,
            super::ActionSourceKind::Otx,
            Some(BusinessMutation::PaymentOutputInRemainder),
        ),
        (_, Some(FillActionCase::TxLevelFillOrder)) => coverage(
            FlowKind::TxLevelAndOtx,
            ScriptRoleKind::InputType,
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::TxLevel,
            Some(BusinessMutation::TxLevelActionInsteadOfOtxAction),
        ),
        (_, Some(FillActionCase::OutputTypeTarget)) => coverage(
            FlowKind::OtxOnly,
            ScriptRoleKind::InputType,
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::WrongTarget,
            Some(BusinessMutation::WrongActionTarget),
        ),
        (_, Some(FillActionCase::PaymentInAnotherOtx)) => coverage(
            FlowKind::OtxOnly,
            ScriptRoleKind::InputType,
            OtxScopeKind::AppendOutput,
            super::ActionSourceKind::Otx,
            Some(BusinessMutation::PaymentOutputInAnotherOtx),
        ),
        (_, Some(FillActionCase::PaymentOutputOutOfRange)) => coverage(
            FlowKind::TxLevelAndOtx,
            ScriptRoleKind::InputType,
            OtxScopeKind::Remainder,
            super::ActionSourceKind::Otx,
            Some(BusinessMutation::PaymentOutputInRemainder),
        ),
        (_, Some(FillActionCase::PaymentOutputWrongUdt)) => {
            fill_mutation(BusinessMutation::PaymentOutputWrongUdt)
        }
        (_, Some(FillActionCase::PaymentOutputWrongOwner)) => {
            fill_mutation(BusinessMutation::PaymentOutputWrongOwner)
        }
        (_, Some(FillActionCase::PaymentOutputInsufficient)) => {
            fill_mutation(BusinessMutation::PaymentOutputInsufficient)
        }
        (_, Some(FillActionCase::MissingBuyerNftOutput)) => {
            fill_mutation(BusinessMutation::BuyerNftMissing)
        }
        (_, Some(FillActionCase::BuyerNftWrongLock)) => {
            fill_mutation(BusinessMutation::BuyerNftWrongLock)
        }
        (_, Some(FillActionCase::BuyerNftWrongType)) => {
            fill_mutation(BusinessMutation::BuyerNftWrongType)
        }
        (
            _,
            Some(
                FillActionCase::TwoTypeOrdersReusePaymentOutput
                | FillActionCase::TwoTypeOrdersUseDistinctPaymentOutputs,
            ),
        ) => LimitOrderHappyPath::TwoTypeOrders.default_coverage(),
    }
}

fn fill_mutation(mutation: BusinessMutation) -> CoverageTag {
    coverage(
        FlowKind::OtxOnly,
        ScriptRoleKind::InputType,
        OtxScopeKind::BaseInput,
        super::ActionSourceKind::Otx,
        Some(mutation),
    )
}

fn create_coverage(case: CreateOrderCase) -> CoverageTag {
    let mutation = match case {
        CreateOrderCase::Valid => None,
        CreateOrderCase::MissingNftProxyOutput => {
            Some(BusinessMutation::CreateMissingNftProxyOutput)
        }
        CreateOrderCase::WrongNftType => Some(BusinessMutation::CreateWrongNftType),
        CreateOrderCase::WrongProxyOrder => Some(BusinessMutation::CreateWrongProxyOrder),
        CreateOrderCase::StateActionMismatch => Some(BusinessMutation::CreateStateActionMismatch),
        CreateOrderCase::InvalidTypeId => Some(BusinessMutation::CreateInvalidTypeId),
        CreateOrderCase::InputAndOutputGroupShape => {
            Some(BusinessMutation::CreateInputAndOutputGroupShape)
        }
    };
    coverage(
        FlowKind::TxLevel,
        ScriptRoleKind::OutputType,
        OtxScopeKind::Remainder,
        super::ActionSourceKind::TxLevel,
        mutation,
    )
}

fn push_deps<'a>(
    shape: &mut TxShape,
    scripts: impl IntoIterator<Item = &'a crate::framework::contracts::DeployedScript>,
) {
    for script in scripts {
        shape.push_prefix_cell_dep(cell_dep_for_script(script));
    }
}

fn type_id_args(first_input: &CellInput, output_index: u64) -> [u8; 32] {
    let mut blake2b = new_blake2b();
    blake2b.update(first_input.as_slice());
    blake2b.update(&output_index.to_le_bytes());
    let mut out = [0u8; 32];
    blake2b.finalize(&mut out);
    out
}
