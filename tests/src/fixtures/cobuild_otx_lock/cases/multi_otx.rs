use super::*;

pub fn two_udt_transfer_otxs_case(include_fee_input: bool) -> BuiltCobuildOtxLockCase {
    let fee_secret_key = fixed_secret_key(1);
    let otx_a_secret_key = fixed_secret_key(2);
    let otx_b_secret_key = fixed_secret_key(3);
    let mut fixture = CobuildTestFixture::new();
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let fee_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&fee_secret_key),
    );
    let otx_a_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&otx_a_secret_key),
    );
    let otx_b_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&otx_b_secret_key),
    );
    let fee_lock_hash = fee_lock.script_hash;
    let otx_a_lock_hash = otx_a_lock.script_hash;
    let otx_b_lock_hash = otx_b_lock.script_hash;
    let issuer_lock = build_cobuild_otx_lock(fixture.context_mut(), &lock_code, &[0u8; 20]);
    let udt_code = deploy_test_udt_code(fixture.context_mut());
    let udt = build_test_udt_script(fixture.context_mut(), &udt_code, issuer_lock.script_hash);

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep);
    shape.push_prefix_cell_dep(udt.cell_dep);
    let fee_input = include_fee_input.then(|| {
        shape.push_prefix_input(live_resolved_facts(
            fixture.context_mut(),
            normal_output(fee_lock.script, 100_000_000_000),
            Bytes::new(),
        ))
    });

    let otx_a = shape.push_otx(OtxSpec {
        base_inputs: vec![live_resolved_facts(
            fixture.context_mut(),
            typed_udt_cell(otx_a_lock.script.clone(), udt.script.clone()),
            udt_amount_data(100),
        )],
        base_outputs: vec![
            udt_output(otx_a_lock.script.clone(), udt.script.clone(), 40),
            udt_output(otx_a_lock.script, udt.script.clone(), 60),
        ],
        base_input_masks: Some(full_base_input_masks(1)),
        base_output_masks: Some(full_base_output_masks(2)),
        ..Default::default()
    });
    let otx_b = shape.push_otx(OtxSpec {
        base_inputs: vec![live_resolved_facts(
            fixture.context_mut(),
            typed_udt_cell(otx_b_lock.script.clone(), udt.script.clone()),
            udt_amount_data(300),
        )],
        base_outputs: vec![
            udt_output(otx_b_lock.script.clone(), udt.script.clone(), 100),
            udt_output(otx_b_lock.script.clone(), udt.script.clone(), 100),
            udt_output(otx_b_lock.script, udt.script, 100),
        ],
        base_input_masks: Some(full_base_input_masks(1)),
        base_output_masks: Some(full_base_output_masks(3)),
        ..Default::default()
    });

    let mut built = shape.build();
    let oracle = TestSigningHashOracle;
    let otx_a_facts = sign_scope(
        &built,
        &oracle,
        SignerId("otx-a"),
        &otx_a_secret_key,
        otx_a_lock_hash,
        built.otx_witness(otx_a),
        SignatureScope::OtxBase { otx: otx_a },
    );
    let otx_b_facts = sign_scope(
        &built,
        &oracle,
        SignerId("otx-b"),
        &otx_b_secret_key,
        otx_b_lock_hash,
        built.otx_witness(otx_b),
        SignatureScope::OtxBase { otx: otx_b },
    );
    fill_otx_seals(&mut built, otx_a, std::slice::from_ref(&otx_a_facts));
    fill_otx_seals(&mut built, otx_b, std::slice::from_ref(&otx_b_facts));
    let mut signing_facts = vec![otx_a_facts, otx_b_facts];

    if include_fee_input {
        let fee_facts = sign_and_fill_tx_level_lock_group(
            &mut built,
            fee_input.expect("fee input"),
            &fee_secret_key,
            fee_lock_hash,
            SignerId("fee-payer"),
        );
        signing_facts.push(fee_facts);
    }

    BuiltCobuildOtxLockCase {
        name: if include_fee_input {
            "contract_accepts_two_udt_transfer_otxs_with_sighash_all_fee_input"
        } else {
            "contract_accepts_two_udt_transfer_otxs_in_one_transaction"
        },
        fixture,
        built,
        signing_facts,
        expected: ExpectedOutcome::Pass,
        two_udt_transfer_facts: Some(TwoUdtTransferFacts {
            fee_lock_hash: include_fee_input.then_some(fee_lock_hash),
            otx_a_lock_hash,
            otx_b_lock_hash,
        }),
    }
}

pub(super) fn nft_for_udt_swap_otxs_case() -> BuiltCobuildOtxLockCase {
    let otx_a_secret_key = fixed_secret_key(4);
    let otx_b_secret_key = fixed_secret_key(5);
    let mut fixture = CobuildTestFixture::new();
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let otx_a_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&otx_a_secret_key),
    );
    let otx_b_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&otx_b_secret_key),
    );
    let otx_a_lock_hash = otx_a_lock.script_hash;
    let otx_b_lock_hash = otx_b_lock.script_hash;
    let issuer_lock = build_cobuild_otx_lock(fixture.context_mut(), &lock_code, &[0u8; 20]);
    let udt_code = deploy_test_udt_code(fixture.context_mut());
    let nft_code = deploy_test_nft_code(fixture.context_mut());
    let udt = build_test_udt_script(fixture.context_mut(), &udt_code, issuer_lock.script_hash);
    let nft = build_test_nft_script(fixture.context_mut(), &nft_code, [0x44; 32]);
    let nft_payload = nft_data(b"swap-nft", [4, 5, 6, 7], 1_717_171_719);

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep);
    shape.push_prefix_cell_dep(udt.cell_dep);
    shape.push_prefix_cell_dep(nft.cell_dep);
    let otx_a = shape.push_otx(OtxSpec {
        base_inputs: vec![live_resolved_facts(
            fixture.context_mut(),
            typed_asset_cell(
                otx_a_lock.script.clone(),
                nft.script.clone(),
                200_000_000_000,
            ),
            nft_payload.clone(),
        )],
        base_outputs: vec![udt_output(otx_a_lock.script, udt.script.clone(), 1000)],
        base_input_masks: Some(full_base_input_masks(1)),
        base_output_masks: Some(full_base_output_masks(1)),
        ..Default::default()
    });
    let otx_b = shape.push_otx(OtxSpec {
        base_inputs: vec![live_resolved_facts(
            fixture.context_mut(),
            typed_udt_cell(otx_b_lock.script.clone(), udt.script.clone()),
            udt_amount_data(1000),
        )],
        base_outputs: vec![TestCellOutput::new(
            typed_asset_cell(otx_b_lock.script, nft.script.clone(), 190_000_000_000),
            nft_payload,
        )],
        base_input_masks: Some(full_base_input_masks(1)),
        base_output_masks: Some(full_base_output_masks(1)),
        ..Default::default()
    });

    let mut built = shape.build();
    let oracle = TestSigningHashOracle;
    let otx_a_facts = sign_scope(
        &built,
        &oracle,
        SignerId("nft-for-udt-swap-a"),
        &otx_a_secret_key,
        otx_a_lock_hash,
        built.otx_witness(otx_a),
        SignatureScope::OtxBase { otx: otx_a },
    );
    let otx_b_facts = sign_scope(
        &built,
        &oracle,
        SignerId("nft-for-udt-swap-b"),
        &otx_b_secret_key,
        otx_b_lock_hash,
        built.otx_witness(otx_b),
        SignatureScope::OtxBase { otx: otx_b },
    );
    fill_otx_seals(&mut built, otx_a, std::slice::from_ref(&otx_a_facts));
    fill_otx_seals(&mut built, otx_b, std::slice::from_ref(&otx_b_facts));

    BuiltCobuildOtxLockCase {
        name: "contract_accepts_nft_for_udt_swap_otxs_in_one_transaction",
        fixture,
        built,
        signing_facts: vec![otx_a_facts, otx_b_facts],
        expected: ExpectedOutcome::Pass,
        two_udt_transfer_facts: None,
    }
}

pub(super) fn nft_for_udt_append_otx_swap_case(coverage_previous: bool) -> BuiltCobuildOtxLockCase {
    let otx_a_secret_key = fixed_secret_key(6);
    let otx_b_secret_key = fixed_secret_key(7);
    let mut fixture = CobuildTestFixture::new();
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let otx_a_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&otx_a_secret_key),
    );
    let otx_b_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&otx_b_secret_key),
    );
    let otx_a_lock_hash = otx_a_lock.script_hash;
    let otx_b_lock_hash = otx_b_lock.script_hash;
    let issuer_lock = build_cobuild_otx_lock(fixture.context_mut(), &lock_code, &[0u8; 20]);
    let udt_code = deploy_test_udt_code(fixture.context_mut());
    let nft_code = deploy_test_nft_code(fixture.context_mut());
    let udt = build_test_udt_script(fixture.context_mut(), &udt_code, issuer_lock.script_hash);
    let nft = build_test_nft_script(fixture.context_mut(), &nft_code, [0x45; 32]);
    let nft_payload = nft_data(b"append-swap-nft", [8, 9, 10, 11], 1_717_171_720);

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep);
    shape.push_prefix_cell_dep(udt.cell_dep);
    shape.push_prefix_cell_dep(nft.cell_dep);
    let otx = shape.push_otx(OtxSpec {
        base_inputs: vec![live_resolved_facts(
            fixture.context_mut(),
            typed_asset_cell(
                otx_a_lock.script.clone(),
                nft.script.clone(),
                200_000_000_000,
            ),
            nft_payload.clone(),
        )],
        base_outputs: vec![udt_output(otx_a_lock.script, udt.script.clone(), 1000)],
        append_segments: vec![
            append_segment_spec(if coverage_previous { 0x02 } else { 0x00 })
                .with_inputs(vec![live_resolved_facts(
                    fixture.context_mut(),
                    typed_udt_cell(otx_b_lock.script.clone(), udt.script.clone()),
                    udt_amount_data(1000),
                )])
                .with_outputs(vec![TestCellOutput::new(
                    typed_asset_cell(otx_b_lock.script, nft.script.clone(), 190_000_000_000),
                    nft_payload,
                )]),
        ],
        base_input_masks: Some(full_base_input_masks(1)),
        base_output_masks: Some(full_base_output_masks(1)),
        ..Default::default()
    });

    let mut built = shape.build();
    let oracle = TestSigningHashOracle;
    let otx_a_facts = sign_scope(
        &built,
        &oracle,
        SignerId("nft-for-udt-append-swap-a"),
        &otx_a_secret_key,
        otx_a_lock_hash,
        built.otx_witness(otx),
        SignatureScope::OtxBase { otx },
    );
    let otx_b_facts = sign_scope(
        &built,
        &oracle,
        SignerId("nft-for-udt-append-swap-b"),
        &otx_b_secret_key,
        otx_b_lock_hash,
        built.otx_witness(otx),
        SignatureScope::OtxAppendSegment {
            otx,
            segment_index: 0,
        },
    );
    fill_otx_seals(&mut built, otx, &[otx_a_facts.clone(), otx_b_facts.clone()]);

    BuiltCobuildOtxLockCase {
        name: if coverage_previous {
            "contract_accepts_nft_for_udt_append_otx_swap_with_previous_coverage"
        } else {
            "contract_accepts_nft_for_udt_append_otx_swap"
        },
        fixture,
        built,
        signing_facts: vec![otx_a_facts, otx_b_facts],
        expected: ExpectedOutcome::Pass,
        two_udt_transfer_facts: None,
    }
}
