use super::*;

pub fn create_minter_case() -> NftMinterCase {
    create_minter_case_with("create_minter", 0, 10, Some(10), CreateExpected::Pass)
}

pub fn create_minter_missing_action_case() -> NftMinterCase {
    create_minter_case_with(
        "create_minter_missing_action",
        0,
        10,
        None,
        CreateExpected::Output(NftMinterTypeError::InvalidCobuild),
    )
}

pub fn create_minter_non_zero_counter_case() -> NftMinterCase {
    create_minter_case_with(
        "create_minter_non_zero_counter",
        1,
        10,
        Some(10),
        CreateExpected::Output(NftMinterTypeError::Counter),
    )
}

pub fn create_minter_supply_cap_mismatch_case() -> NftMinterCase {
    create_minter_case_with(
        "create_minter_supply_cap_mismatch",
        0,
        9,
        Some(10),
        CreateExpected::Output(NftMinterTypeError::SupplyCap),
    )
}

pub fn create_minter_real_sighash_all_signed_case() -> NftMinterCase {
    create_minter_real_sighash_all_case(
        "create_minter_real_sighash_all_signed",
        RealSighashAllMode::Valid,
    )
}

pub fn create_minter_real_sighash_all_bad_seal_case() -> NftMinterCase {
    create_minter_real_sighash_all_case(
        "create_minter_real_sighash_all_bad_seal",
        RealSighashAllMode::BadSeal,
    )
}

pub fn create_minter_real_sighash_all_tampered_output_case() -> NftMinterCase {
    create_minter_real_sighash_all_case(
        "create_minter_real_sighash_all_tampered_output",
        RealSighashAllMode::TamperOutput,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CreateExpected {
    Pass,
    Output(NftMinterTypeError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RealSighashAllMode {
    Valid,
    BadSeal,
    TamperOutput,
}

fn create_minter_real_sighash_all_case(
    name: &'static str,
    mode: RealSighashAllMode,
) -> NftMinterCase {
    let secret_key = fixed_secret_key(43);
    let mut fixture = CobuildTestFixture::new();
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &lock_code,
        &public_key_hash20(&secret_key),
    );
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), Vec::new());
    let funding_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(lock.script.clone(), 200_000_000_000),
        Bytes::new(),
    );
    let minter_script = rebuild_data2_script(
        fixture.context_mut(),
        &minter_code,
        type_id_args(&funding_input.input, 0).to_vec(),
    );
    let minter_hash = script_hash(&minter_script);
    let output = TestCellOutput::new(
        typed_output(lock.script.clone(), minter_script.clone(), 200_000_000_000),
        minter_data(MinterState {
            mint_counter: 0,
            supply_cap: 10,
        }),
    );
    let message = CobuildMessageBuilder::new()
        .output_type_action(minter_hash)
        .action_data(create_minter_action_data(10))
        .build();

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    let funding_input = shape.push_prefix_input(funding_input);
    let minter_output = shape.push_remainder_output(output);
    shape.tx_level_message(message.clone());
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);

    let oracle = TestSigningHashOracle;
    let signing_hash = oracle.tx_with_message(&built, &message);
    let mut seal = sign_recoverable(&secret_key, signing_hash);
    if mode == RealSighashAllMode::BadSeal {
        seal[0] ^= 0x01;
    }
    let witness = WitnessLayout::from(
        SighashAll::new_builder()
            .seal(seal)
            .message(message)
            .build(),
    );
    built.apply_shape_mutation(TxShapeMutation::ReplaceWitness {
        witness: built.tx_level_witness(),
        replacement: Bytes::copy_from_slice(witness.as_slice()),
    });

    if mode == RealSighashAllMode::TamperOutput {
        built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
            output: minter_output,
            replacement: TestCellOutput::new(
                typed_output(lock.script.clone(), minter_script, 200_000_000_001),
                minter_data(MinterState {
                    mint_counter: 0,
                    supply_cap: 10,
                }),
            ),
        });
    }

    let expected = match mode {
        RealSighashAllMode::Valid => NftMinterExpected::Pass,
        RealSighashAllMode::BadSeal | RealSighashAllMode::TamperOutput => {
            NftMinterExpected::OtxLockInput {
                input: funding_input,
                error: CobuildOtxLockError::BadSeal,
            }
        }
    };

    NftMinterCase {
        name,
        fixture,
        built,
        expected,
    }
}

fn create_minter_case_with(
    name: &'static str,
    counter: u64,
    output_cap: u64,
    action_cap: Option<u64>,
    expected: CreateExpected,
) -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), Vec::new());
    let funding_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(lock.script.clone(), 200_000_000_000),
        Bytes::new(),
    );
    let minter_script = rebuild_data2_script(
        fixture.context_mut(),
        &minter_code,
        type_id_args(&funding_input.input, 0).to_vec(),
    );
    let minter_hash = script_hash(&minter_script);
    let output = TestCellOutput::new(
        typed_output(lock.script.clone(), minter_script, 200_000_000_000),
        minter_data(MinterState {
            mint_counter: counter,
            supply_cap: output_cap,
        }),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_prefix_input(funding_input);
    let minter_output = shape.push_remainder_output(output);
    if let Some(action_cap) = action_cap {
        shape.tx_level_message(
            CobuildMessageBuilder::new()
                .output_type_action(minter_hash)
                .action_data(create_minter_action_data(action_cap))
                .build(),
        );
    }
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    let expected = match expected {
        CreateExpected::Pass => NftMinterExpected::Pass,
        CreateExpected::Output(error) => NftMinterExpected::MinterOutputType {
            output: minter_output,
            error,
        },
    };
    NftMinterCase {
        name,
        fixture,
        built,
        expected,
    }
}
