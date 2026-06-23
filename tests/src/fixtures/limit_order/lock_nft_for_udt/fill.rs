use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OrderInputScope {
    Base,
    Append,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AssetChoice {
    Expected,
    Wrong,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NftOutputKind {
    Expected,
    Missing,
    WrongLock,
    WrongType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PaymentHandleSource {
    CurrentOtx,
    Remainder,
    AnotherOtx,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LockActionKind {
    Fill,
    UnknownTag,
    MalformedFill,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LockScenario {
    name: &'static str,
    happy_path: LimitOrderHappyPath,
    mutation: Option<BusinessMutation>,
    expected_error: Option<LimitOrderLockError>,
    coverage: CoverageTag,
    malformed_lock_args: bool,
    input_nft: AssetChoice,
    payment_udt: AssetChoice,
    payment_lock: AssetChoice,
    payment_amount: u128,
    nft_output: NftOutputKind,
    order_input_scope: OrderInputScope,
    tx_level_message: bool,
    action_in_tx_level: bool,
    tx_level_action_noise: bool,
    tx_level_action_target: AssetChoice,
    action_target: AssetChoice,
    payment_handle: PaymentHandleSource,
    remainder_payment_amount: Option<u128>,
    other_otx_payment_amount: Option<u128>,
    action: LockActionKind,
}

impl LockScenario {
    fn happy(name: &'static str) -> Self {
        Self {
            name,
            happy_path: LimitOrderHappyPath::LockNftForUdt,
            mutation: None,
            expected_error: None,
            coverage: LimitOrderHappyPath::LockNftForUdt.default_coverage(),
            malformed_lock_args: false,
            input_nft: AssetChoice::Expected,
            payment_udt: AssetChoice::Expected,
            payment_lock: AssetChoice::Expected,
            payment_amount: 30,
            nft_output: NftOutputKind::Expected,
            order_input_scope: OrderInputScope::Base,
            tx_level_message: false,
            action_in_tx_level: false,
            tx_level_action_noise: false,
            tx_level_action_target: AssetChoice::Expected,
            action_target: AssetChoice::Expected,
            payment_handle: PaymentHandleSource::CurrentOtx,
            remainder_payment_amount: None,
            other_otx_payment_amount: None,
            action: LockActionKind::Fill,
        }
    }

    fn mutated(
        name: &'static str,
        mutation: BusinessMutation,
        expected_error: LimitOrderLockError,
        coverage: CoverageTag,
    ) -> Self {
        Self {
            name,
            mutation: Some(mutation),
            expected_error: Some(expected_error),
            coverage,
            ..Self::happy(name)
        }
    }
}

pub(super) fn lock_script_fill_cases() -> Vec<BuiltLimitOrderCase> {
    lock_fill_scenarios()
        .into_iter()
        .map(lock_nft_for_udt_case)
        .collect()
}

fn lock_fill_scenarios() -> Vec<LockScenario> {
    vec![
        LockScenario::happy("Valid"),
        LockScenario {
            name: "SighashAll",
            tx_level_message: true,
            coverage: coverage(
                FlowKind::TxLevelAndOtx,
                ScriptRoleKind::InputLock,
                OtxScopeKind::BaseInput,
                super::ActionSourceKind::Otx,
                None,
            ),
            ..LockScenario::happy("SighashAll")
        },
        LockScenario {
            name: "TxLevelAndOtxFillOrder",
            tx_level_message: true,
            tx_level_action_noise: true,
            mutation: Some(BusinessMutation::TxLevelAndOtxDuplicateAction),
            expected_error: Some(LimitOrderLockError::InvalidAction),
            coverage: coverage(
                FlowKind::TxLevelAndOtx,
                ScriptRoleKind::InputLock,
                OtxScopeKind::BaseInput,
                super::ActionSourceKind::Duplicate,
                Some(BusinessMutation::TxLevelAndOtxDuplicateAction),
            ),
            ..LockScenario::happy("TxLevelAndOtxFillOrder")
        },
        LockScenario {
            name: "TxLevelNoiseAndOtxFillOrder",
            tx_level_message: true,
            tx_level_action_noise: true,
            tx_level_action_target: AssetChoice::Wrong,
            coverage: coverage(
                FlowKind::TxLevelAndOtx,
                ScriptRoleKind::InputLock,
                OtxScopeKind::BaseInput,
                super::ActionSourceKind::Otx,
                None,
            ),
            ..LockScenario::happy("TxLevelNoiseAndOtxFillOrder")
        },
        LockScenario {
            malformed_lock_args: true,
            ..lock_mutation(
                "MalformedArgs",
                BusinessMutation::MalformedLockArgs,
                LimitOrderLockError::MalformedArgs,
                OtxScopeKind::BaseInput,
                super::ActionSourceKind::Otx,
            )
        },
        LockScenario {
            input_nft: AssetChoice::Wrong,
            ..lock_mutation(
                "WrongNftType",
                BusinessMutation::WrongNftType,
                LimitOrderLockError::WrongNftType,
                OtxScopeKind::BaseInput,
                super::ActionSourceKind::Otx,
            )
        },
        LockScenario {
            tx_level_message: true,
            action_in_tx_level: true,
            ..lock_mutation(
                "TxLevelFillOrder",
                BusinessMutation::TxLevelActionInsteadOfOtxAction,
                LimitOrderLockError::InvalidAction,
                OtxScopeKind::BaseInput,
                super::ActionSourceKind::TxLevel,
            )
        },
        LockScenario {
            action_target: AssetChoice::Wrong,
            ..lock_mutation(
                "WrongActionTarget",
                BusinessMutation::WrongActionTarget,
                LimitOrderLockError::InvalidAction,
                OtxScopeKind::BaseInput,
                super::ActionSourceKind::WrongTarget,
            )
        },
        LockScenario {
            order_input_scope: OrderInputScope::Append,
            ..lock_mutation(
                "OrderInputInAppendScope",
                BusinessMutation::OrderInputInAppendScope,
                LimitOrderLockError::InvalidAction,
                OtxScopeKind::AppendInput,
                super::ActionSourceKind::Otx,
            )
        },
        LockScenario {
            payment_amount: 29,
            ..payment_mutation(
                "InsufficientUdt",
                BusinessMutation::PaymentOutputInsufficient,
                LimitOrderLockError::InvalidPayment,
            )
        },
        LockScenario {
            payment_udt: AssetChoice::Wrong,
            ..payment_mutation(
                "WrongUdt",
                BusinessMutation::PaymentOutputWrongUdt,
                LimitOrderLockError::InvalidPayment,
            )
        },
        LockScenario {
            payment_lock: AssetChoice::Wrong,
            ..payment_mutation(
                "WrongOwner",
                BusinessMutation::PaymentOutputWrongOwner,
                LimitOrderLockError::InvalidPayment,
            )
        },
        LockScenario {
            payment_amount: 29,
            remainder_payment_amount: Some(1),
            ..lock_mutation(
                "TxLevelRemainderOnly",
                BusinessMutation::PaymentOutputInRemainder,
                LimitOrderLockError::InvalidPayment,
                OtxScopeKind::Remainder,
                super::ActionSourceKind::Otx,
            )
        },
        LockScenario {
            payment_amount: 29,
            payment_handle: PaymentHandleSource::AnotherOtx,
            other_otx_payment_amount: Some(1),
            ..lock_mutation(
                "PaymentInAnotherOtx",
                BusinessMutation::PaymentOutputInAnotherOtx,
                LimitOrderLockError::InvalidAction,
                OtxScopeKind::AppendOutput,
                super::ActionSourceKind::Otx,
            )
        },
        LockScenario {
            payment_handle: PaymentHandleSource::Remainder,
            remainder_payment_amount: Some(30),
            ..lock_mutation(
                "PaymentOutputOutOfRange",
                BusinessMutation::PaymentOutputInRemainder,
                LimitOrderLockError::InvalidAction,
                OtxScopeKind::Remainder,
                super::ActionSourceKind::Otx,
            )
        },
        LockScenario {
            payment_udt: AssetChoice::Wrong,
            ..payment_mutation(
                "PaymentOutputWrongUdt",
                BusinessMutation::PaymentOutputWrongUdt,
                LimitOrderLockError::InvalidPayment,
            )
        },
        LockScenario {
            payment_lock: AssetChoice::Wrong,
            ..payment_mutation(
                "PaymentOutputWrongOwner",
                BusinessMutation::PaymentOutputWrongOwner,
                LimitOrderLockError::InvalidPayment,
            )
        },
        LockScenario {
            payment_amount: 29,
            ..payment_mutation(
                "PaymentOutputInsufficient",
                BusinessMutation::PaymentOutputInsufficient,
                LimitOrderLockError::InvalidPayment,
            )
        },
        LockScenario {
            nft_output: NftOutputKind::Missing,
            ..base_output_mutation("MissingBuyerNftOutput", BusinessMutation::BuyerNftMissing)
        },
        LockScenario {
            nft_output: NftOutputKind::WrongLock,
            ..base_output_mutation("BuyerNftWrongLock", BusinessMutation::BuyerNftWrongLock)
        },
        LockScenario {
            nft_output: NftOutputKind::WrongType,
            ..base_output_mutation("BuyerNftWrongType", BusinessMutation::BuyerNftWrongType)
        },
        LockScenario {
            action: LockActionKind::UnknownTag,
            ..lock_mutation(
                "UnknownActionTag",
                BusinessMutation::UnknownActionTag,
                LimitOrderLockError::UnknownActionTag,
                OtxScopeKind::BaseInput,
                super::ActionSourceKind::Otx,
            )
        },
        LockScenario {
            action: LockActionKind::MalformedFill,
            ..lock_mutation(
                "MalformedAction",
                BusinessMutation::MalformedAction,
                LimitOrderLockError::MalformedAction,
                OtxScopeKind::BaseInput,
                super::ActionSourceKind::Otx,
            )
        },
    ]
}

fn lock_mutation(
    name: &'static str,
    mutation: BusinessMutation,
    expected_error: LimitOrderLockError,
    otx_scope: OtxScopeKind,
    action_source: super::ActionSourceKind,
) -> LockScenario {
    LockScenario::mutated(
        name,
        mutation,
        expected_error,
        fill_mutation(otx_scope, action_source, mutation),
    )
}

fn payment_mutation(
    name: &'static str,
    mutation: BusinessMutation,
    expected_error: LimitOrderLockError,
) -> LockScenario {
    lock_mutation(
        name,
        mutation,
        expected_error,
        OtxScopeKind::AppendOutput,
        super::ActionSourceKind::Otx,
    )
}

fn base_output_mutation(name: &'static str, mutation: BusinessMutation) -> LockScenario {
    lock_mutation(
        name,
        mutation,
        LimitOrderLockError::InvalidAction,
        OtxScopeKind::BaseOutput,
        super::ActionSourceKind::Otx,
    )
}

fn lock_nft_for_udt_case(scenario: LockScenario) -> BuiltLimitOrderCase {
    let mut fixture = CobuildTestFixture::new();
    let limit_order_lock_code = deploy_limit_order_lock(fixture.context_mut());
    let owner_success = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let buyer_success = deploy_always_success(fixture.context_mut(), b"buyer".to_vec());
    let owner_lock = owner_success.script.clone();
    let buyer_lock = buyer_success.script.clone();
    let issuer_lock_hash = script_hash(&owner_success.script);
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
        scenario.malformed_lock_args,
    );
    let order_lock_hash = script_hash(&order_lock);
    let input_nft = match scenario.input_nft {
        AssetChoice::Expected => nft.clone(),
        AssetChoice::Wrong => wrong_nft.clone(),
    };
    let payment_udt = match scenario.payment_udt {
        AssetChoice::Expected => udt.clone(),
        AssetChoice::Wrong => wrong_udt.clone(),
    };
    let payment_lock = match scenario.payment_lock {
        AssetChoice::Expected => owner_lock.clone(),
        AssetChoice::Wrong => wrong_owner_lock,
    };

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
    let nft_output = match scenario.nft_output {
        NftOutputKind::Missing => TestCellOutput::new(
            normal_output(owner_success.script.clone(), 90_000_000_000),
            Vec::new(),
        ),
        NftOutputKind::WrongLock => TestCellOutput::new(
            typed_output(wrong_buyer_lock, nft.script.clone(), 90_000_000_000),
            nft_payload.clone(),
        ),
        NftOutputKind::WrongType => TestCellOutput::new(
            typed_output(buyer_lock.clone(), wrong_nft.script.clone(), 90_000_000_000),
            nft_payload.clone(),
        ),
        NftOutputKind::Expected => TestCellOutput::new(
            typed_output(buyer_lock.clone(), input_nft.script.clone(), 90_000_000_000),
            nft_payload.clone(),
        ),
    };
    let udt_payment_output = TestCellOutput::new(
        typed_output(payment_lock, payment_udt.script.clone(), 90_000_000_000),
        udt_amount_data(scenario.payment_amount),
    );
    let dummy_base_input = (scenario.order_input_scope == OrderInputScope::Append
        || scenario.other_otx_payment_amount.is_some())
    .then(|| {
        live_resolved_facts(
            fixture.context_mut(),
            normal_output(owner_success.script.clone(), 100_000_000_000),
            Vec::new(),
        )
    });
    let remainder_payment_output = scenario.remainder_payment_amount.map(|amount| {
        TestCellOutput::new(
            typed_output(owner_lock.clone(), udt.script.clone(), 90_000_000_000),
            udt_amount_data(amount),
        )
    });
    let other_otx_payment_output = scenario.other_otx_payment_amount.map(|amount| {
        TestCellOutput::new(
            typed_output(owner_lock.clone(), udt.script.clone(), 90_000_000_000),
            udt_amount_data(amount),
        )
    });

    let mut shape = TxShape::new();
    push_deps(
        &mut shape,
        [
            &limit_order_lock_code,
            &owner_success,
            &buyer_success,
            &wrong_owner,
            &nft,
            &wrong_nft,
            &udt,
            &wrong_udt,
        ],
    );
    let otx = match scenario.order_input_scope {
        OrderInputScope::Append => shape.push_otx(OtxSpec {
            base_inputs: vec![dummy_base_input.clone().expect("dummy base input")],
            base_outputs: vec![nft_output],
            append_segments: vec![
                append_segment_spec(0x00)
                    .with_inputs(vec![nft_input, udt_input])
                    .with_outputs(vec![udt_payment_output])
                    .with_seals(vec![empty_lock_seal(order_lock_hash)]),
            ],
            ..Default::default()
        }),
        OrderInputScope::Base => shape.push_otx(OtxSpec {
            base_inputs: vec![nft_input],
            base_outputs: vec![nft_output],
            append_segments: vec![
                append_segment_spec(0x00)
                    .with_inputs(vec![udt_input])
                    .with_outputs(vec![udt_payment_output]),
            ],
            base_seals: vec![empty_lock_seal(order_lock_hash)],
            ..Default::default()
        }),
    };
    let seal_scope = match scenario.order_input_scope {
        OrderInputScope::Append => SignatureScope::OtxAppendSegment {
            otx,
            segment_index: 0,
        },
        OrderInputScope::Base => SignatureScope::OtxBase { otx },
    };
    let order_input = match scenario.order_input_scope {
        OrderInputScope::Append => shape.otx_append_input(otx, 0),
        OrderInputScope::Base => shape.otx_base_input(otx, 0),
    };
    let current_payment = shape.otx_append_output(otx, 0);
    let remainder_payment =
        remainder_payment_output.map(|output| shape.push_remainder_output(output));
    let other_payment = if let Some(output) = other_otx_payment_output {
        let other_otx = shape.push_otx(OtxSpec {
            base_inputs: vec![dummy_base_input.expect("dummy base input")],
            append_segments: vec![append_segment_spec(0x00).with_outputs(vec![output])],
            ..Default::default()
        });
        Some(shape.otx_append_output(other_otx, 0))
    } else {
        None
    };
    if scenario.tx_level_message {
        shape.tx_level_message(empty_message());
    }

    let mut built = shape.build();
    let payment = match scenario.payment_handle {
        PaymentHandleSource::CurrentOtx => current_payment,
        PaymentHandleSource::Remainder => remainder_payment.expect("remainder payment"),
        PaymentHandleSource::AnotherOtx => other_payment.expect("other OTX payment"),
    };
    let action = match scenario.action {
        LockActionKind::UnknownTag => LimitOrderAction::UnknownTag,
        LockActionKind::MalformedFill => LimitOrderAction::MalformedFill {
            payment,
            buyer_lock_hash: script_hash(&buyer_lock),
        },
        LockActionKind::Fill => LimitOrderAction::Fill {
            payment,
            buyer_lock_hash: script_hash(&buyer_lock),
        },
    };
    let target = match scenario.action_target {
        AssetChoice::Expected => order_lock_hash,
        AssetChoice::Wrong => [8; 32],
    };
    let message = fill_message(&fixture, target, action, &built);
    if scenario.action_in_tx_level {
        replace_tx_level_message(&mut built, message);
    } else if scenario.tx_level_action_noise {
        let tx_level_target = match scenario.tx_level_action_target {
            AssetChoice::Expected => target,
            AssetChoice::Wrong => [8; 32],
        };
        let tx_level_message = fill_message(&fixture, tx_level_target, action, &built);
        replace_tx_level_message(&mut built, tx_level_message);
        replace_otx_message(&mut built, otx, message);
    } else {
        replace_otx_message(&mut built, otx, message);
    }

    let signing_facts = vec![empty_seal_facts(&built, order_lock_hash, seal_scope)];
    let expected = lock_fill_expected(&scenario, order_input);

    built_case(
        format!("lock_fill::{}", scenario.name),
        fixture,
        built,
        signing_facts,
        expected,
        scenario.coverage,
    )
}

fn lock_fill_expected(scenario: &LockScenario, input: InputHandle) -> LimitOrderExpectedOutcome {
    scenario
        .expected_error
        .map(|error| input_lock_error(input, error))
        .unwrap_or(LimitOrderExpectedOutcome::Pass)
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
