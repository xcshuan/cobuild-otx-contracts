use super::*;

pub fn mint_mixed_tx_and_otx_order_case() -> NftMinterCase {
    let minter_secret_key = fixed_secret_key(70);
    let mut fixture = CobuildTestFixture::new();
    let always_success_code = deploy_always_success_code(fixture.context_mut());
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let minter_type_code = deploy_nft_minter_type_code(fixture.context_mut());
    let minted_nft_type_code = deploy_minted_nft_type_code(fixture.context_mut());
    let lock = build_always_success_script(
        fixture.context_mut(),
        &always_success_code,
        b"owner".to_vec(),
    );
    let minter_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&minter_secret_key),
    );
    let minter_script =
        build_nft_minter_type_script(fixture.context_mut(), &minter_type_code, [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_script.script);
    let nft_6_id = nft_id(minter_hash, 6);
    let nft_7_id = nft_id(minter_hash, 7);
    let nft_6_script =
        build_minted_nft_type_script(fixture.context_mut(), &minted_nft_type_code, nft_6_id);
    let nft_7_script =
        build_minted_nft_type_script(fixture.context_mut(), &minted_nft_type_code, nft_7_id);
    let (minter_input, minter_output) = minter_transition(
        &mut fixture,
        &minter_lock.script,
        &minter_script.script,
        6,
        8,
    );
    let base_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(lock.script.clone(), 100_000_000_000),
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep.clone());
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_6_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_7_script.cell_dep.clone());
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    let tx_message = CobuildMessageBuilder::new()
        .input_type_action(minter_hash)
        .action_data(mint_nft_action_data([6u8; 32], script_hash(&lock.script)))
        .build();
    shape.tx_level_message(tx_message.clone());
    shape.push_otx(OtxSpec {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data([7u8; 32], script_hash(&lock.script)))
                .build(),
        ),
        base_inputs: vec![base_input],
        append_segments: vec![
            append_segment_spec(0x00).with_outputs(vec![minted_nft_output(
                &lock.script,
                &nft_7_script.script,
                minter_hash,
                7,
                [7u8; 32],
            )]),
        ],
        ..Default::default()
    });
    shape.push_remainder_output(minted_nft_output(
        &lock.script,
        &nft_6_script.script,
        minter_hash,
        6,
        [6u8; 32],
    ));
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    sign_tx_with_message_input(&mut built, minter_input, &tx_message, &minter_secret_key);
    NftMinterCase {
        name: "mint_mixed_tx_and_otx_order",
        fixture,
        built,
        expected: NftMinterExpected::Pass,
    }
}

pub fn mint_otx_output_in_base_range_case() -> NftMinterCase {
    let minter_secret_key = fixed_secret_key(71);
    let mut fixture = CobuildTestFixture::new();
    let always_success_code = deploy_always_success_code(fixture.context_mut());
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let minter_type_code = deploy_nft_minter_type_code(fixture.context_mut());
    let minted_nft_type_code = deploy_minted_nft_type_code(fixture.context_mut());
    let lock = build_always_success_script(
        fixture.context_mut(),
        &always_success_code,
        b"owner".to_vec(),
    );
    let minter_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&minter_secret_key),
    );
    let minter_script =
        build_nft_minter_type_script(fixture.context_mut(), &minter_type_code, [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_script.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_id = nft_id(minter_hash, serial);
    let nft_script =
        build_minted_nft_type_script(fixture.context_mut(), &minted_nft_type_code, nft_id);
    let (minter_input, minter_output) = minter_transition(
        &mut fixture,
        &minter_lock.script,
        &minter_script.script,
        serial,
        serial + 1,
    );
    let base_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(lock.script.clone(), 100_000_000_000),
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep.clone());
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_script.cell_dep.clone());
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    shape.push_otx(OtxSpec {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(seed, script_hash(&lock.script)))
                .build(),
        ),
        base_inputs: vec![base_input],
        base_outputs: vec![minted_nft_output(
            &lock.script,
            &nft_script.script,
            minter_hash,
            serial,
            seed,
        )],
        ..Default::default()
    });
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    sign_tx_without_message_input(
        &mut built,
        minter_input,
        &minter_secret_key,
        minter_lock.script_hash,
        SignerId("nft_minter_state_owner"),
    );
    NftMinterCase {
        name: "mint_otx_output_in_base_range",
        fixture,
        built,
        expected: NftMinterExpected::Pass,
    }
}

pub fn mint_otx_output_in_remainder_case() -> NftMinterCase {
    let minter_secret_key = fixed_secret_key(72);
    let mut fixture = CobuildTestFixture::new();
    let always_success_code = deploy_always_success_code(fixture.context_mut());
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let minter_type_code = deploy_nft_minter_type_code(fixture.context_mut());
    let minted_nft_type_code = deploy_minted_nft_type_code(fixture.context_mut());
    let lock = build_always_success_script(
        fixture.context_mut(),
        &always_success_code,
        b"owner".to_vec(),
    );
    let minter_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&minter_secret_key),
    );
    let minter_script =
        build_nft_minter_type_script(fixture.context_mut(), &minter_type_code, [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_script.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_script = build_minted_nft_type_script(
        fixture.context_mut(),
        &minted_nft_type_code,
        nft_id(minter_hash, serial),
    );
    let (minter_input, minter_output) = minter_transition(
        &mut fixture,
        &minter_lock.script,
        &minter_script.script,
        serial,
        serial + 1,
    );
    let base_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(lock.script.clone(), 100_000_000_000),
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep.clone());
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_script.cell_dep.clone());
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    let otx = shape.push_otx(OtxSpec {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(seed, script_hash(&lock.script)))
                .build(),
        ),
        base_inputs: vec![base_input],
        ..Default::default()
    });
    let _ = otx;
    shape.push_remainder_output(minted_nft_output(
        &lock.script,
        &nft_script.script,
        minter_hash,
        serial,
        seed,
    ));
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    sign_tx_without_message_input(
        &mut built,
        minter_input,
        &minter_secret_key,
        minter_lock.script_hash,
        SignerId("nft_minter_state_owner"),
    );
    NftMinterCase {
        name: "mint_otx_output_in_remainder",
        fixture,
        built,
        expected: NftMinterExpected::MinterInputType {
            input: minter_input,
            error: NftMinterTypeError::InvalidMintedNft,
        },
    }
}

pub fn mint_otx_output_in_other_otx_append_range_case() -> NftMinterCase {
    let minter_secret_key = fixed_secret_key(73);
    let mut fixture = CobuildTestFixture::new();
    let always_success_code = deploy_always_success_code(fixture.context_mut());
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let minter_type_code = deploy_nft_minter_type_code(fixture.context_mut());
    let minted_nft_type_code = deploy_minted_nft_type_code(fixture.context_mut());
    let lock = build_always_success_script(
        fixture.context_mut(),
        &always_success_code,
        b"owner".to_vec(),
    );
    let minter_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&minter_secret_key),
    );
    let minter_script =
        build_nft_minter_type_script(fixture.context_mut(), &minter_type_code, [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_script.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_script = build_minted_nft_type_script(
        fixture.context_mut(),
        &minted_nft_type_code,
        nft_id(minter_hash, serial),
    );
    let (minter_input, minter_output) = minter_transition(
        &mut fixture,
        &minter_lock.script,
        &minter_script.script,
        serial,
        serial + 1,
    );
    let action_base_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(lock.script.clone(), 100_000_000_000),
        Bytes::new(),
    );
    let unrelated_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(lock.script.clone(), 200_000_000_000),
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep.clone());
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_script.cell_dep.clone());
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    shape.push_otx(OtxSpec {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(seed, script_hash(&lock.script)))
                .build(),
        ),
        base_inputs: vec![action_base_input],
        ..Default::default()
    });
    shape.push_otx(OtxSpec {
        base_inputs: vec![unrelated_input],
        append_segments: vec![
            append_segment_spec(0x00).with_outputs(vec![minted_nft_output(
                &lock.script,
                &nft_script.script,
                minter_hash,
                serial,
                seed,
            )]),
        ],
        ..Default::default()
    });
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    sign_tx_without_message_input(
        &mut built,
        minter_input,
        &minter_secret_key,
        minter_lock.script_hash,
        SignerId("nft_minter_state_owner"),
    );
    NftMinterCase {
        name: "mint_otx_output_in_other_otx_append_range",
        fixture,
        built,
        expected: NftMinterExpected::MinterInputType {
            input: minter_input,
            error: NftMinterTypeError::InvalidMintedNft,
        },
    }
}

pub fn mint_real_otx_lock_signed_base_case() -> NftMinterCase {
    real_otx_lock_mint_case("mint_real_otx_lock_signed_base", RealOtxLockMintMode::Valid)
}

pub fn mint_real_otx_lock_base_nft_output_lock_capacity_mask_case() -> NftMinterCase {
    real_otx_lock_base_nft_output_case(
        "mint_real_otx_lock_base_nft_output_lock_capacity_mask",
        RealOtxLockBaseNftOutputMode::Valid,
    )
}

pub fn mint_real_otx_lock_base_nft_output_tampered_capacity_case() -> NftMinterCase {
    real_otx_lock_base_nft_output_case(
        "mint_real_otx_lock_base_nft_output_tampered_capacity",
        RealOtxLockBaseNftOutputMode::TamperNftCapacity,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RealOtxLockBaseNftOutputMode {
    Valid,
    TamperNftCapacity,
}

fn real_otx_lock_base_nft_output_case(
    name: &'static str,
    mode: RealOtxLockBaseNftOutputMode,
) -> NftMinterCase {
    let user_secret_key = fixed_secret_key(43);
    let minter_secret_key = fixed_secret_key(46);
    let mut fixture = CobuildTestFixture::new();
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let user_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&user_secret_key),
    );
    let minter_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&minter_secret_key),
    );
    let minter_type_code = deploy_nft_minter_type_code(fixture.context_mut());
    let minted_nft_type_code = deploy_minted_nft_type_code(fixture.context_mut());
    let minter_script =
        build_nft_minter_type_script(fixture.context_mut(), &minter_type_code, [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_script.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_script = build_minted_nft_type_script(
        fixture.context_mut(),
        &minted_nft_type_code,
        nft_id(minter_hash, serial),
    );
    let (minter_input, minter_output) = minter_transition(
        &mut fixture,
        &minter_lock.script,
        &minter_script.script,
        serial,
        serial + 1,
    );
    let minted_output = minted_nft_output(
        &user_lock.script,
        &nft_script.script,
        minter_hash,
        serial,
        seed,
    );
    let user_base_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(user_lock.script.clone(), 100_000_000_000),
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_script.cell_dep.clone());
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    let otx = shape.push_otx(OtxSpec {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(seed, script_hash(&user_lock.script)))
                .build(),
        ),
        base_inputs: vec![user_base_input],
        base_outputs: vec![minted_output.clone()],
        base_output_masks: Some(base_output_masks(
            1,
            &[
                (0, BaseOutputMaskField::Capacity),
                (0, BaseOutputMaskField::Lock),
            ],
        )),
        ..Default::default()
    });
    let user_base_input = shape.otx_base_input(otx, 0);
    let minted_base_output = shape.otx_base_output(otx, 0);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);

    let facts = sign_scope(
        &built,
        &TestSigningHashOracle,
        SignerId("nft_minter_owner"),
        &user_secret_key,
        user_lock.script_hash,
        built.otx_witness(otx),
        SignatureScope::OtxBase { otx },
    );
    built.apply_protocol_mutation(ProtocolMutation::BaseSealRaw {
        otx,
        script_hash: user_lock.script_hash,
        seal: Some(facts.seal),
    });

    if mode == RealOtxLockBaseNftOutputMode::TamperNftCapacity {
        built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
            output: minted_base_output,
            replacement: TestCellOutput::new(
                typed_output(
                    user_lock.script.clone(),
                    nft_script.script.clone(),
                    200_000_000_001,
                ),
                minted_output.data.clone(),
            ),
        });
    }
    sign_tx_without_message_input(
        &mut built,
        minter_input,
        &minter_secret_key,
        minter_lock.script_hash,
        SignerId("nft_minter_state_owner"),
    );

    let expected = match mode {
        RealOtxLockBaseNftOutputMode::Valid => NftMinterExpected::Pass,
        RealOtxLockBaseNftOutputMode::TamperNftCapacity => NftMinterExpected::OtxLockInput {
            input: user_base_input,
            error: CobuildOtxLockError::BadSeal,
        },
    };

    NftMinterCase {
        name,
        fixture,
        built,
        expected,
    }
}

pub fn mint_real_otx_lock_tampered_base_output_case() -> NftMinterCase {
    real_otx_lock_mint_case(
        "mint_real_otx_lock_tampered_base_output",
        RealOtxLockMintMode::TamperBaseOutput,
    )
}

pub fn mint_real_otx_lock_tampered_append_nft_output_signed_base_case() -> NftMinterCase {
    real_otx_lock_mint_case(
        "mint_real_otx_lock_tampered_append_nft_output_signed_base",
        RealOtxLockMintMode::TamperAppendNftOutputSignedBase,
    )
}

pub fn mint_real_otx_lock_missing_base_seal_case() -> NftMinterCase {
    real_otx_lock_mint_case(
        "mint_real_otx_lock_missing_base_seal",
        RealOtxLockMintMode::MissingBaseSeal,
    )
}

pub fn mint_real_otx_lock_bad_base_seal_case() -> NftMinterCase {
    real_otx_lock_mint_case(
        "mint_real_otx_lock_bad_base_seal",
        RealOtxLockMintMode::BadBaseSeal,
    )
}

pub fn mint_real_otx_lock_bad_append_seal_case() -> NftMinterCase {
    let user_secret_key = fixed_secret_key(43);
    let minter_secret_key = fixed_secret_key(44);
    let mut fixture = CobuildTestFixture::new();
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let always_success_code = deploy_always_success_code(fixture.context_mut());
    let user_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&user_secret_key),
    );
    let minter_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&minter_secret_key),
    );
    let base_lock = build_always_success_script(
        fixture.context_mut(),
        &always_success_code,
        b"bad-append-base".to_vec(),
    );
    let minter_type_code = deploy_nft_minter_type_code(fixture.context_mut());
    let minted_nft_type_code = deploy_minted_nft_type_code(fixture.context_mut());
    let minter_script =
        build_nft_minter_type_script(fixture.context_mut(), &minter_type_code, [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_script.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_script = build_minted_nft_type_script(
        fixture.context_mut(),
        &minted_nft_type_code,
        nft_id(minter_hash, serial),
    );
    let (minter_input, minter_output) = minter_transition(
        &mut fixture,
        &minter_lock.script,
        &minter_script.script,
        serial,
        serial + 1,
    );
    let minted_output = minted_nft_output(
        &user_lock.script,
        &nft_script.script,
        minter_hash,
        serial,
        seed,
    );
    let base_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(base_lock.script.clone(), 100_000_000_000),
        Bytes::new(),
    );
    let user_append_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(user_lock.script.clone(), 100_000_000_000),
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep.clone());
    shape.push_prefix_cell_dep(base_lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_script.cell_dep.clone());
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    let otx = shape.push_otx(OtxSpec {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(seed, script_hash(&user_lock.script)))
                .build(),
        ),
        base_inputs: vec![base_input],
        append_segments: vec![
            append_segment_spec(0x00)
                .with_inputs(vec![user_append_input])
                .with_outputs(vec![minted_output]),
        ],
        ..Default::default()
    });
    let user_append_input = shape.otx_append_input(otx, 0);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);

    let oracle = TestSigningHashOracle;
    let append_facts = sign_scope(
        &built,
        &oracle,
        SignerId("nft_minter_append_owner"),
        &user_secret_key,
        user_lock.script_hash,
        built.otx_witness(otx),
        SignatureScope::OtxAppendSegment {
            otx,
            segment_index: 0,
        },
    );
    let mut seal = append_facts.seal;
    seal[0] ^= 0x01;
    built.apply_protocol_mutation(ProtocolMutation::AppendSegmentSealRaw {
        otx,
        segment_index: 0,
        script_hash: user_lock.script_hash,
        seal: Some(seal),
    });
    sign_tx_without_message_input(
        &mut built,
        minter_input,
        &minter_secret_key,
        minter_lock.script_hash,
        SignerId("nft_minter_state_owner"),
    );

    NftMinterCase {
        name: "mint_real_otx_lock_bad_append_seal",
        fixture,
        built,
        expected: NftMinterExpected::OtxLockInput {
            input: user_append_input,
            error: CobuildOtxLockError::BadSeal,
        },
    }
}

pub fn mint_three_otx_actions_single_minter_transition_signed_base_case() -> NftMinterCase {
    let minter_secret_key = fixed_secret_key(50);
    let secret_key_a = fixed_secret_key(51);
    let secret_key_b = fixed_secret_key(52);
    let secret_key_c = fixed_secret_key(53);
    let mut fixture = CobuildTestFixture::new();
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let user_lock_a = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&secret_key_a),
    );
    let user_lock_b = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&secret_key_b),
    );
    let user_lock_c = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&secret_key_c),
    );
    let minter_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&minter_secret_key),
    );
    let minter_type_code = deploy_nft_minter_type_code(fixture.context_mut());
    let minted_nft_type_code = deploy_minted_nft_type_code(fixture.context_mut());
    let minter_script =
        build_nft_minter_type_script(fixture.context_mut(), &minter_type_code, [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_script.script);
    let nft_6_script = build_minted_nft_type_script(
        fixture.context_mut(),
        &minted_nft_type_code,
        nft_id(minter_hash, 6),
    );
    let nft_7_script = build_minted_nft_type_script(
        fixture.context_mut(),
        &minted_nft_type_code,
        nft_id(minter_hash, 7),
    );
    let nft_8_script = build_minted_nft_type_script(
        fixture.context_mut(),
        &minted_nft_type_code,
        nft_id(minter_hash, 8),
    );
    let (minter_input, minter_output) = minter_transition(
        &mut fixture,
        &minter_lock.script,
        &minter_script.script,
        6,
        9,
    );
    let user_a_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(user_lock_a.script.clone(), 200_000_000_000),
        Bytes::new(),
    );
    let user_b_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(user_lock_b.script.clone(), 200_000_000_000),
        Bytes::new(),
    );
    let user_c_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(user_lock_c.script.clone(), 200_000_000_000),
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_6_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_7_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_8_script.cell_dep.clone());
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    let otx_a = shape.push_otx(OtxSpec {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(
                    [6u8; 32],
                    script_hash(&user_lock_a.script),
                ))
                .build(),
        ),
        base_inputs: vec![user_a_input],
        append_segments: vec![
            append_segment_spec(0x00).with_outputs(vec![minted_nft_output(
                &user_lock_a.script,
                &nft_6_script.script,
                minter_hash,
                6,
                [6u8; 32],
            )]),
        ],
        ..Default::default()
    });
    let otx_b = shape.push_otx(OtxSpec {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(
                    [7u8; 32],
                    script_hash(&user_lock_b.script),
                ))
                .build(),
        ),
        base_inputs: vec![user_b_input],
        append_segments: vec![
            append_segment_spec(0x00).with_outputs(vec![minted_nft_output(
                &user_lock_b.script,
                &nft_7_script.script,
                minter_hash,
                7,
                [7u8; 32],
            )]),
        ],
        ..Default::default()
    });
    let otx_c = shape.push_otx(OtxSpec {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(
                    [8u8; 32],
                    script_hash(&user_lock_c.script),
                ))
                .build(),
        ),
        base_inputs: vec![user_c_input],
        append_segments: vec![
            append_segment_spec(0x00).with_outputs(vec![minted_nft_output(
                &user_lock_c.script,
                &nft_8_script.script,
                minter_hash,
                8,
                [8u8; 32],
            )]),
        ],
        ..Default::default()
    });
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);

    // One minter transition advances 6 -> 9 while three OTX messages provide
    // the three mint actions. OTX B/C are action and append-output carriers.
    let oracle = TestSigningHashOracle;
    for (otx, script_hash, signer, secret_key) in [
        (
            otx_a,
            user_lock_a.script_hash,
            SignerId("nft_minter_owner_a"),
            &secret_key_a,
        ),
        (
            otx_b,
            user_lock_b.script_hash,
            SignerId("nft_minter_owner_b"),
            &secret_key_b,
        ),
        (
            otx_c,
            user_lock_c.script_hash,
            SignerId("nft_minter_owner_c"),
            &secret_key_c,
        ),
    ] {
        let facts = sign_scope(
            &built,
            &oracle,
            signer,
            secret_key,
            script_hash,
            built.otx_witness(otx),
            SignatureScope::OtxBase { otx },
        );
        built.apply_protocol_mutation(ProtocolMutation::BaseSealRaw {
            otx,
            script_hash,
            seal: Some(facts.seal),
        });
    }
    sign_tx_without_message_input(
        &mut built,
        minter_input,
        &minter_secret_key,
        minter_lock.script_hash,
        SignerId("nft_minter_state_owner"),
    );

    NftMinterCase {
        name: "mint_three_otx_actions_single_minter_transition_signed_base",
        fixture,
        built,
        expected: NftMinterExpected::Pass,
    }
}

pub fn mint_three_otx_actions_single_minter_transition_signed_append_case() -> NftMinterCase {
    let minter_secret_key = fixed_secret_key(60);
    let secret_key_a = fixed_secret_key(61);
    let secret_key_b = fixed_secret_key(62);
    let secret_key_c = fixed_secret_key(63);
    let mut fixture = CobuildTestFixture::new();
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let always_success_code = deploy_always_success_code(fixture.context_mut());
    let user_lock_a = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&secret_key_a),
    );
    let user_lock_b = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&secret_key_b),
    );
    let user_lock_c = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&secret_key_c),
    );
    let minter_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&minter_secret_key),
    );
    let base_lock = build_always_success_script(
        fixture.context_mut(),
        &always_success_code,
        b"signed-append-base".to_vec(),
    );
    let minter_type_code = deploy_nft_minter_type_code(fixture.context_mut());
    let minted_nft_type_code = deploy_minted_nft_type_code(fixture.context_mut());
    let minter_script =
        build_nft_minter_type_script(fixture.context_mut(), &minter_type_code, [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_script.script);
    let nft_6_script = build_minted_nft_type_script(
        fixture.context_mut(),
        &minted_nft_type_code,
        nft_id(minter_hash, 6),
    );
    let nft_7_script = build_minted_nft_type_script(
        fixture.context_mut(),
        &minted_nft_type_code,
        nft_id(minter_hash, 7),
    );
    let nft_8_script = build_minted_nft_type_script(
        fixture.context_mut(),
        &minted_nft_type_code,
        nft_id(minter_hash, 8),
    );
    let (minter_input, minter_output) = minter_transition(
        &mut fixture,
        &minter_lock.script,
        &minter_script.script,
        6,
        9,
    );
    let user_a_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(user_lock_a.script.clone(), 200_000_000_000),
        Bytes::new(),
    );
    let user_b_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(user_lock_b.script.clone(), 200_000_000_000),
        Bytes::new(),
    );
    let user_c_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(user_lock_c.script.clone(), 200_000_000_000),
        Bytes::new(),
    );
    let base_input_a = live_resolved_facts(
        fixture.context_mut(),
        normal_output(base_lock.script.clone(), 100_000_000_000),
        Bytes::new(),
    );
    let base_input_b = live_resolved_facts(
        fixture.context_mut(),
        normal_output(base_lock.script.clone(), 100_000_000_000),
        Bytes::new(),
    );
    let base_input_c = live_resolved_facts(
        fixture.context_mut(),
        normal_output(base_lock.script.clone(), 100_000_000_000),
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep.clone());
    shape.push_prefix_cell_dep(base_lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_6_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_7_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_8_script.cell_dep.clone());
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    let otx_a = shape.push_otx(OtxSpec {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(
                    [6u8; 32],
                    script_hash(&user_lock_a.script),
                ))
                .build(),
        ),
        base_inputs: vec![base_input_a],
        append_segments: vec![
            append_segment_spec(0x00)
                .with_inputs(vec![user_a_input])
                .with_outputs(vec![minted_nft_output(
                    &user_lock_a.script,
                    &nft_6_script.script,
                    minter_hash,
                    6,
                    [6u8; 32],
                )]),
        ],
        ..Default::default()
    });
    let otx_b = shape.push_otx(OtxSpec {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(
                    [7u8; 32],
                    script_hash(&user_lock_b.script),
                ))
                .build(),
        ),
        base_inputs: vec![base_input_b],
        append_segments: vec![
            append_segment_spec(0x00)
                .with_inputs(vec![user_b_input])
                .with_outputs(vec![minted_nft_output(
                    &user_lock_b.script,
                    &nft_7_script.script,
                    minter_hash,
                    7,
                    [7u8; 32],
                )]),
        ],
        ..Default::default()
    });
    let otx_c = shape.push_otx(OtxSpec {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(
                    [8u8; 32],
                    script_hash(&user_lock_c.script),
                ))
                .build(),
        ),
        base_inputs: vec![base_input_c],
        append_segments: vec![
            append_segment_spec(0x00)
                .with_inputs(vec![user_c_input])
                .with_outputs(vec![minted_nft_output(
                    &user_lock_c.script,
                    &nft_8_script.script,
                    minter_hash,
                    8,
                    [8u8; 32],
                )]),
        ],
        ..Default::default()
    });
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);

    let oracle = TestSigningHashOracle;
    for (otx, script_hash, signer, secret_key) in [
        (
            otx_a,
            user_lock_a.script_hash,
            SignerId("nft_minter_append_owner_a"),
            &secret_key_a,
        ),
        (
            otx_b,
            user_lock_b.script_hash,
            SignerId("nft_minter_append_owner_b"),
            &secret_key_b,
        ),
        (
            otx_c,
            user_lock_c.script_hash,
            SignerId("nft_minter_append_owner_c"),
            &secret_key_c,
        ),
    ] {
        let facts = sign_scope(
            &built,
            &oracle,
            signer,
            secret_key,
            script_hash,
            built.otx_witness(otx),
            SignatureScope::OtxAppendSegment {
                otx,
                segment_index: 0,
            },
        );
        built.apply_protocol_mutation(ProtocolMutation::AppendSegmentSealRaw {
            otx,
            segment_index: 0,
            script_hash,
            seal: Some(facts.seal),
        });
    }
    sign_tx_without_message_input(
        &mut built,
        minter_input,
        &minter_secret_key,
        minter_lock.script_hash,
        SignerId("nft_minter_state_owner"),
    );

    NftMinterCase {
        name: "mint_three_otx_actions_single_minter_transition_signed_append",
        fixture,
        built,
        expected: NftMinterExpected::Pass,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RealOtxLockMintMode {
    Valid,
    TamperBaseOutput,
    TamperAppendNftOutputSignedBase,
    MissingBaseSeal,
    BadBaseSeal,
}

fn real_otx_lock_mint_case(name: &'static str, mode: RealOtxLockMintMode) -> NftMinterCase {
    let user_secret_key = fixed_secret_key(42);
    let minter_secret_key = fixed_secret_key(45);
    let mut fixture = CobuildTestFixture::new();
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let user_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&user_secret_key),
    );
    let minter_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&minter_secret_key),
    );
    let minter_type_code = deploy_nft_minter_type_code(fixture.context_mut());
    let minted_nft_type_code = deploy_minted_nft_type_code(fixture.context_mut());
    let minter_script =
        build_nft_minter_type_script(fixture.context_mut(), &minter_type_code, [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_script.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_script = build_minted_nft_type_script(
        fixture.context_mut(),
        &minted_nft_type_code,
        nft_id(minter_hash, serial),
    );
    let (minter_input, minter_output) = minter_transition(
        &mut fixture,
        &minter_lock.script,
        &minter_script.script,
        serial,
        serial + 1,
    );
    let minted_output = minted_nft_output(
        &user_lock.script,
        &nft_script.script,
        minter_hash,
        serial,
        seed,
    );
    let user_base_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(user_lock.script.clone(), 100_000_000_000),
        Bytes::new(),
    );
    let user_base_output = TestCellOutput::new(
        normal_output(user_lock.script.clone(), 90_000_000_000),
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_script.cell_dep.clone());
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    let otx = shape.push_otx(OtxSpec {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(seed, script_hash(&user_lock.script)))
                .build(),
        ),
        base_inputs: vec![user_base_input],
        base_outputs: vec![user_base_output],
        append_segments: vec![append_segment_spec(0x00).with_outputs(vec![minted_output.clone()])],
        ..Default::default()
    });
    let user_base_input = shape.otx_base_input(otx, 0);
    let user_base_output = shape.otx_base_output(otx, 0);
    let minted_append_output = shape.otx_append_output(otx, 0);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);

    let oracle = TestSigningHashOracle;
    let base_facts = sign_scope(
        &built,
        &oracle,
        SignerId("nft_minter_owner"),
        &user_secret_key,
        user_lock.script_hash,
        built.otx_witness(otx),
        SignatureScope::OtxBase { otx },
    );
    if mode != RealOtxLockMintMode::MissingBaseSeal {
        let mut seal = base_facts.seal;
        if mode == RealOtxLockMintMode::BadBaseSeal {
            seal[0] ^= 0x01;
        }
        built.apply_protocol_mutation(ProtocolMutation::BaseSealRaw {
            otx,
            script_hash: user_lock.script_hash,
            seal: Some(seal),
        });
    }
    if mode == RealOtxLockMintMode::TamperBaseOutput {
        built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
            output: user_base_output,
            replacement: TestCellOutput::new(
                normal_output(user_lock.script.clone(), 90_000_000_001),
                Bytes::new(),
            ),
        });
    }
    if mode == RealOtxLockMintMode::TamperAppendNftOutputSignedBase {
        let always_success_code = deploy_always_success_code(fixture.context_mut());
        let replacement_lock = build_always_success_script(
            fixture.context_mut(),
            &always_success_code,
            b"append-owner".to_vec(),
        );
        built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
            output: minted_append_output,
            replacement: minted_nft_output(
                &replacement_lock.script,
                &nft_script.script,
                minter_hash,
                serial,
                seed,
            ),
        });
    }
    sign_tx_without_message_input(
        &mut built,
        minter_input,
        &minter_secret_key,
        minter_lock.script_hash,
        SignerId("nft_minter_state_owner"),
    );

    let expected = match mode {
        RealOtxLockMintMode::Valid => NftMinterExpected::Pass,
        RealOtxLockMintMode::TamperAppendNftOutputSignedBase => {
            NftMinterExpected::MinterInputType {
                input: minter_input,
                error: NftMinterTypeError::InvalidMintedNft,
            }
        }
        RealOtxLockMintMode::TamperBaseOutput | RealOtxLockMintMode::BadBaseSeal => {
            NftMinterExpected::OtxLockInput {
                input: user_base_input,
                error: CobuildOtxLockError::BadSeal,
            }
        }
        RealOtxLockMintMode::MissingBaseSeal => NftMinterExpected::OtxLockInput {
            input: user_base_input,
            error: CobuildOtxLockError::MissingLockSeal,
        },
    };

    NftMinterCase {
        name,
        fixture,
        built,
        expected,
    }
}
