use super::*;

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
    TxLevelAndOtxFillOrder,
    TxLevelNoiseAndOtxFillOrder,
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
        nft_for_udt_case(NftForUdtScenario::action(
            FillActionCase::TxLevelAndOtxFillOrder,
        )),
        nft_for_udt_case(NftForUdtScenario::action(
            FillActionCase::TxLevelNoiseAndOtxFillOrder,
        )),
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

fn nft_for_udt_case(scenario: NftForUdtScenario) -> BuiltLimitOrderCase {
    let mut fixture = CobuildTestFixture::new();
    let limit_order = fixture.deploy_limit_order();
    let owner_success = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let buyer_success = deploy_always_success(fixture.context_mut(), b"buyer".to_vec());
    let owner_lock = owner_success.script.clone();
    let buyer_lock = buyer_success.script.clone();
    let issuer_lock_hash = script_hash(&owner_success.script);
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
            normal_output(owner_success.script.clone(), 90_000_000_000),
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
            &owner_success,
            &buyer_success,
            &proxy_lock,
            &nft,
            &wrong_nft,
            &udt,
            &wrong_udt,
        ],
    );
    let otx = shape.push_otx(OtxSpec {
        base_inputs: vec![order_input, nft_input],
        base_outputs: vec![nft_output],
        append_segments: vec![
            append_segment_spec(0x00)
                .with_inputs(vec![udt_input])
                .with_outputs(vec![udt_payment_output]),
        ],
        ..Default::default()
    });
    let order = shape.otx_base_input(otx, 0);
    let buyer_nft_output = shape.otx_base_output(otx, 0);
    let current_payment = shape.otx_append_output(otx, 0);
    let remainder_payment =
        remainder_payment_output.map(|output| shape.push_remainder_output(output));
    let other_payment = if let Some(output) = other_otx_payment_output {
        let dummy_input = live_resolved_facts(
            fixture.context_mut(),
            normal_output(owner_success.script.clone(), 100_000_000_000),
            Vec::new(),
        );
        let other_otx = shape.push_otx(OtxSpec {
            base_inputs: vec![dummy_input],
            append_segments: vec![append_segment_spec(0x00).with_outputs(vec![output])],
            ..Default::default()
        });
        Some(shape.otx_append_output(other_otx, 0))
    } else {
        None
    };
    if scenario.sighash_all
        || matches!(
            scenario.action_case,
            Some(
                FillActionCase::TxLevelFillOrder
                    | FillActionCase::TxLevelAndOtxFillOrder
                    | FillActionCase::TxLevelNoiseAndOtxFillOrder
            )
        )
    {
        shape.tx_level_message(empty_message());
    }
    let mut built = shape.build();
    let payment = match scenario.action_case {
        Some(FillActionCase::PaymentInAnotherOtx) => other_payment.expect("other OTX payment"),
        Some(FillActionCase::PaymentOutputOutOfRange) => {
            remainder_payment.expect("remainder payment")
        }
        None if scenario.payment_case == NftForUdtPaymentCase::TxLevelRemainderOnly => {
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
    } else if matches!(
        scenario.action_case,
        Some(FillActionCase::TxLevelAndOtxFillOrder | FillActionCase::TxLevelNoiseAndOtxFillOrder)
    ) {
        let tx_level_target =
            if scenario.action_case == Some(FillActionCase::TxLevelNoiseAndOtxFillOrder) {
                [8; 32]
            } else {
                limit_order.script_hash
            };
        let tx_level_message = fill_message(
            &fixture,
            tx_level_target,
            payment,
            script_hash(&buyer_lock),
            &built,
        );
        replace_tx_level_message(&mut built, tx_level_message);
        replace_otx_message(&mut built, otx, message);
    } else {
        replace_otx_message(&mut built, otx, message);
    }

    let expected = match (scenario.payment_case, scenario.action_case) {
        (NftForUdtPaymentCase::Valid, None) => LimitOrderExpectedOutcome::Pass,
        (NftForUdtPaymentCase::Valid, Some(FillActionCase::TxLevelNoiseAndOtxFillOrder)) => {
            LimitOrderExpectedOutcome::Pass
        }
        (
            NftForUdtPaymentCase::Valid,
            Some(
                FillActionCase::PaymentOutputWrongUdt
                | FillActionCase::PaymentOutputWrongOwner
                | FillActionCase::PaymentOutputInsufficient,
            ),
        ) => input_type_error(order, LimitOrderTypeError::InvalidPayment),
        (NftForUdtPaymentCase::Valid, Some(FillActionCase::BuyerNftWrongType)) => {
            LimitOrderExpectedOutcome::Framework(ExpectedOutcome::AnyOf(vec![
                ExpectedOutcome::ScriptExit {
                    location: ScriptLocation::InputType(order),
                    code: LimitOrderTypeError::InvalidAction.code(),
                },
                ExpectedOutcome::ScriptExit {
                    location: ScriptLocation::OutputType(buyer_nft_output),
                    code: 8,
                },
            ]))
        }
        (NftForUdtPaymentCase::Valid, Some(_))
        | (NftForUdtPaymentCase::TxLevelRemainderOnly, None) => {
            input_type_error(order, LimitOrderTypeError::InvalidAction)
        }
        _ => input_type_error(order, LimitOrderTypeError::InvalidPayment),
    };

    built_case(
        format!("fill::{scenario:?}"),
        fixture,
        built,
        expected,
        fill_coverage(scenario),
    )
}

fn two_type_orders_case(case: FillActionCase) -> BuiltLimitOrderCase {
    let mut fixture = CobuildTestFixture::new();
    let limit_order_code = fixture.deploy_limit_order();
    let owner_success = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let buyer_success = deploy_always_success(fixture.context_mut(), b"buyer".to_vec());
    let owner_lock = owner_success.script.clone();
    let buyer_lock = buyer_success.script.clone();
    let issuer_lock_hash = script_hash(&owner_success.script);
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
            &owner_success,
            &buyer_success,
            &proxy_lock_a,
            &proxy_lock_b,
            &nft_a,
            &nft_b,
            &udt,
        ],
    );
    let otx = shape.push_otx(OtxSpec {
        base_inputs: vec![order_input_a, nft_input_a, order_input_b, nft_input_b],
        base_outputs: vec![nft_output_a, nft_output_b],
        append_segments: vec![
            append_segment_spec(0x00)
                .with_inputs(vec![udt_input])
                .with_outputs(vec![payment_output_a, payment_output_b]),
        ],
        ..Default::default()
    });
    let order_a = shape.otx_base_input(otx, 0);
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
        format!("two_type_orders::{case:?}"),
        fixture,
        built,
        if case == FillActionCase::TwoTypeOrdersReusePaymentOutput {
            input_type_error(order_a, LimitOrderTypeError::InvalidAction)
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
        (_, Some(FillActionCase::TxLevelAndOtxFillOrder)) => coverage(
            FlowKind::TxLevelAndOtx,
            ScriptRoleKind::InputType,
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::Duplicate,
            Some(BusinessMutation::TxLevelAndOtxDuplicateAction),
        ),
        (_, Some(FillActionCase::TxLevelNoiseAndOtxFillOrder)) => coverage(
            FlowKind::TxLevelAndOtx,
            ScriptRoleKind::InputType,
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::Otx,
            None,
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
