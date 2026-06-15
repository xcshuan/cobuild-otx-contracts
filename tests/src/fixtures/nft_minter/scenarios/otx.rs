use super::*;

pub fn mint_mixed_tx_and_otx_order_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_code.script);
    let nft_6_id = nft_id(minter_hash, 6);
    let nft_7_id = nft_id(minter_hash, 7);
    let nft_6_code = deploy_minted_nft_type(fixture.context_mut(), nft_6_id);
    let nft_7_code = deploy_minted_nft_type(fixture.context_mut(), nft_7_id);
    let minter_cell = typed_output(
        lock.script.clone(),
        minter_code.script.clone(),
        200_000_000_000,
    );
    let minter_input = live_resolved_facts(
        fixture.context_mut(),
        minter_cell.clone(),
        minter_data(MinterState {
            mint_counter: 6,
            supply_cap: 100,
        }),
    );
    let minter_output = TestCellOutput::new(
        minter_cell,
        minter_data(MinterState {
            mint_counter: 8,
            supply_cap: 100,
        }),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_6_code.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_7_code.cell_dep.clone());
    shape.tx_level_message(
        CobuildMessageBuilder::new()
            .input_type_action(minter_hash)
            .action_data(mint_nft_action_data([6u8; 32], script_hash(&lock.script)))
            .build(),
    );
    shape.push_otx(OtxSegment {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data([7u8; 32], script_hash(&lock.script)))
                .build(),
        ),
        base_inputs: vec![minter_input],
        base_outputs: vec![minter_output],
        append_outputs: vec![minted_nft_output(
            &lock.script,
            &nft_7_code.script,
            minter_hash,
            7,
            [7u8; 32],
        )],
        ..Default::default()
    });
    shape.push_remainder_output(minted_nft_output(
        &lock.script,
        &nft_6_code.script,
        minter_hash,
        6,
        [6u8; 32],
    ));
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "mint_mixed_tx_and_otx_order",
        fixture,
        built,
        expected: NftMinterExpected::Pass,
    }
}

pub fn mint_otx_output_outside_append_range_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_code.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_id = nft_id(minter_hash, serial);
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), nft_id);
    let minter_cell = typed_output(
        lock.script.clone(),
        minter_code.script.clone(),
        200_000_000_000,
    );
    let minter_input = live_resolved_facts(
        fixture.context_mut(),
        minter_cell.clone(),
        minter_data(MinterState {
            mint_counter: serial,
            supply_cap: 100,
        }),
    );
    let minter_output = TestCellOutput::new(
        minter_cell,
        minter_data(MinterState {
            mint_counter: serial + 1,
            supply_cap: 100,
        }),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    let otx = shape.push_otx(OtxSegment {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(seed, script_hash(&lock.script)))
                .build(),
        ),
        base_inputs: vec![minter_input],
        base_outputs: vec![
            minter_output,
            minted_nft_output(&lock.script, &nft_code.script, minter_hash, serial, seed),
        ],
        ..Default::default()
    });
    let minter_input = shape.otx_base_input(otx, 0);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "mint_otx_output_outside_append_range",
        fixture,
        built,
        expected: NftMinterExpected::MinterInputType {
            input: minter_input,
            error: NftMinterTypeError::InvalidMintedNft,
        },
    }
}

pub fn mint_otx_output_in_remainder_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_code.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), nft_id(minter_hash, serial));
    let minter_cell = typed_output(
        lock.script.clone(),
        minter_code.script.clone(),
        200_000_000_000,
    );
    let minter_input = live_resolved_facts(
        fixture.context_mut(),
        minter_cell.clone(),
        minter_data(MinterState {
            mint_counter: serial,
            supply_cap: 100,
        }),
    );
    let minter_output = TestCellOutput::new(
        minter_cell,
        minter_data(MinterState {
            mint_counter: serial + 1,
            supply_cap: 100,
        }),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    let otx = shape.push_otx(OtxSegment {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(seed, script_hash(&lock.script)))
                .build(),
        ),
        base_inputs: vec![minter_input],
        base_outputs: vec![minter_output],
        ..Default::default()
    });
    let minter_input = shape.otx_base_input(otx, 0);
    shape.push_remainder_output(minted_nft_output(
        &lock.script,
        &nft_code.script,
        minter_hash,
        serial,
        seed,
    ));
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
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
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_code.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), nft_id(minter_hash, serial));
    let minter_cell = typed_output(
        lock.script.clone(),
        minter_code.script.clone(),
        200_000_000_000,
    );
    let minter_input = live_resolved_facts(
        fixture.context_mut(),
        minter_cell.clone(),
        minter_data(MinterState {
            mint_counter: serial,
            supply_cap: 100,
        }),
    );
    let minter_output = TestCellOutput::new(
        minter_cell,
        minter_data(MinterState {
            mint_counter: serial + 1,
            supply_cap: 100,
        }),
    );
    let unrelated_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(lock.script.clone(), 200_000_000_000),
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    let otx = shape.push_otx(OtxSegment {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(seed, script_hash(&lock.script)))
                .build(),
        ),
        base_inputs: vec![minter_input],
        base_outputs: vec![minter_output],
        ..Default::default()
    });
    let minter_input = shape.otx_base_input(otx, 0);
    shape.push_otx(OtxSegment {
        base_inputs: vec![unrelated_input],
        append_outputs: vec![minted_nft_output(
            &lock.script,
            &nft_code.script,
            minter_hash,
            serial,
            seed,
        )],
        ..Default::default()
    });
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
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

pub fn mint_three_otx_actions_single_minter_transition_signed_base_case() -> NftMinterCase {
    let secret_key_a = fixed_secret_key(51);
    let secret_key_b = fixed_secret_key(52);
    let secret_key_c = fixed_secret_key(53);
    let mut fixture = CobuildTestFixture::new();
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let user_lock_a = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        0,
        &public_key_hash20(&secret_key_a),
    );
    let user_lock_b = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        0,
        &public_key_hash20(&secret_key_b),
    );
    let user_lock_c = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        0,
        &public_key_hash20(&secret_key_c),
    );
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_code.script);
    let nft_6_code = deploy_minted_nft_type(fixture.context_mut(), nft_id(minter_hash, 6));
    let nft_7_code = deploy_minted_nft_type(fixture.context_mut(), nft_id(minter_hash, 7));
    let nft_8_code = deploy_minted_nft_type(fixture.context_mut(), nft_id(minter_hash, 8));
    let minter_cell = typed_output(
        user_lock_a.script.clone(),
        minter_code.script.clone(),
        200_000_000_000,
    );
    let minter_input = live_resolved_facts(
        fixture.context_mut(),
        minter_cell.clone(),
        minter_data(MinterState {
            mint_counter: 6,
            supply_cap: 100,
        }),
    );
    let minter_output = TestCellOutput::new(
        minter_cell,
        minter_data(MinterState {
            mint_counter: 9,
            supply_cap: 100,
        }),
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
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_6_code.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_7_code.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_8_code.cell_dep.clone());
    let otx_a = shape.push_otx(OtxSegment {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(
                    [6u8; 32],
                    script_hash(&user_lock_a.script),
                ))
                .build(),
        ),
        base_inputs: vec![minter_input],
        base_outputs: vec![minter_output],
        append_outputs: vec![minted_nft_output(
            &user_lock_a.script,
            &nft_6_code.script,
            minter_hash,
            6,
            [6u8; 32],
        )],
        ..Default::default()
    });
    let otx_b = shape.push_otx(OtxSegment {
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
        append_outputs: vec![minted_nft_output(
            &user_lock_b.script,
            &nft_7_code.script,
            minter_hash,
            7,
            [7u8; 32],
        )],
        ..Default::default()
    });
    let otx_c = shape.push_otx(OtxSegment {
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
        append_outputs: vec![minted_nft_output(
            &user_lock_c.script,
            &nft_8_code.script,
            minter_hash,
            8,
            [8u8; 32],
        )],
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
        built.apply_protocol_mutation(ProtocolMutation::SealRaw {
            otx,
            script_hash,
            scope: 0,
            seal: facts.seal,
        });
    }

    NftMinterCase {
        name: "mint_three_otx_actions_single_minter_transition_signed_base",
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
    let secret_key = fixed_secret_key(42);
    let mut fixture = CobuildTestFixture::new();
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let user_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        0,
        &public_key_hash20(&secret_key),
    );
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_code.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), nft_id(minter_hash, serial));
    let minter_cell = typed_output(
        user_lock.script.clone(),
        minter_code.script.clone(),
        200_000_000_000,
    );
    let minter_input = live_resolved_facts(
        fixture.context_mut(),
        minter_cell.clone(),
        minter_data(MinterState {
            mint_counter: serial,
            supply_cap: 100,
        }),
    );
    let minter_output = TestCellOutput::new(
        minter_cell,
        minter_data(MinterState {
            mint_counter: serial + 1,
            supply_cap: 100,
        }),
    );
    let minted_output = minted_nft_output(
        &user_lock.script,
        &nft_code.script,
        minter_hash,
        serial,
        seed,
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    let otx = shape.push_otx(OtxSegment {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(seed, script_hash(&user_lock.script)))
                .build(),
        ),
        base_inputs: vec![minter_input],
        base_outputs: vec![minter_output],
        append_outputs: vec![minted_output.clone()],
        ..Default::default()
    });
    let minter_input = shape.otx_base_input(otx, 0);
    let minter_base_output = shape.otx_base_output(otx, 0);
    let minted_append_output = shape.otx_append_output(otx, 0);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);

    let oracle = TestSigningHashOracle;
    let base_facts = sign_scope(
        &built,
        &oracle,
        SignerId("nft_minter_owner"),
        &secret_key,
        user_lock.script_hash,
        built.otx_witness(otx),
        SignatureScope::OtxBase { otx },
    );
    if mode != RealOtxLockMintMode::MissingBaseSeal {
        let mut seal = base_facts.seal;
        if mode == RealOtxLockMintMode::BadBaseSeal {
            seal[0] ^= 0x01;
        }
        built.apply_protocol_mutation(ProtocolMutation::SealRaw {
            otx,
            script_hash: user_lock.script_hash,
            scope: 0,
            seal,
        });
    }
    if mode == RealOtxLockMintMode::TamperBaseOutput {
        built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
            output: minter_base_output,
            replacement: TestCellOutput::new(
                typed_output(
                    user_lock.script.clone(),
                    minter_code.script.clone(),
                    200_000_000_001,
                ),
                minter_data(MinterState {
                    mint_counter: serial + 1,
                    supply_cap: 100,
                }),
            ),
        });
    }
    if mode == RealOtxLockMintMode::TamperAppendNftOutputSignedBase {
        let replacement_lock =
            deploy_always_success(fixture.context_mut(), b"append-owner".to_vec());
        built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
            output: minted_append_output,
            replacement: minted_nft_output(
                &replacement_lock.script,
                &nft_code.script,
                minter_hash,
                serial,
                seed,
            ),
        });
    }

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
                input: minter_input,
                error: CobuildOtxLockError::BadSeal,
            }
        }
        RealOtxLockMintMode::MissingBaseSeal => NftMinterExpected::OtxLockInput {
            input: minter_input,
            error: CobuildOtxLockError::MissingSealPair,
        },
    };

    NftMinterCase {
        name,
        fixture,
        built,
        expected,
    }
}
