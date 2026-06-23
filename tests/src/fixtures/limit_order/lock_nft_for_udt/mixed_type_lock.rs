use super::*;

pub fn mixed_type_lock_cases() -> Vec<BuiltLimitOrderCase> {
    vec![mixed_type_lock_duplicate_payment_case()]
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
        base_outputs: vec![type_nft_output, lock_nft_output],
        append_segments: vec![
            append_segment_spec(0x00)
                .with_inputs(vec![udt_input])
                .with_outputs(vec![payment_output]),
        ],
        base_seals: vec![empty_lock_seal(order_lock_hash)],
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
