use super::*;

pub fn mint_first_nft_case() -> NftMinterCase {
    mint_from_counter_case("mint_first_nft", 0, 1, [9u8; 32])
}

pub fn mint_from_counter_six_case() -> NftMinterCase {
    mint_from_counter_case("mint_from_counter_six", 6, 7, [6u8; 32])
}

pub fn mint_serial_seven_case() -> NftMinterCase {
    mint_from_counter_case("mint_serial_seven", 7, 8, [7u8; 32])
}

pub fn mint_serial_eleven_case() -> NftMinterCase {
    mint_from_counter_case("mint_serial_eleven", 11, 12, [11u8; 32])
}

pub fn mint_serial_seventy_seven_case() -> NftMinterCase {
    mint_from_counter_case("mint_serial_seventy_seven", 77, 78, [77u8; 32])
}

fn mint_from_counter_case(
    name: &'static str,
    old_counter: u64,
    new_counter: u64,
    seed: [u8; 32],
) -> NftMinterCase {
    mint_from_counter_case_with(
        name,
        old_counter,
        new_counter,
        100,
        seed,
        Some(seed),
        MintExpected::Pass,
    )
}

pub fn mint_wrong_counter_case() -> NftMinterCase {
    mint_from_counter_case_with(
        "mint_wrong_counter",
        6,
        8,
        100,
        [6u8; 32],
        Some([6u8; 32]),
        MintExpected::Input(NftMinterTypeError::Counter),
    )
}

pub fn mint_supply_cap_overrun_case() -> NftMinterCase {
    mint_from_counter_case_with(
        "mint_supply_cap_overrun",
        100,
        101,
        100,
        [100u8; 32],
        Some([100u8; 32]),
        MintExpected::Input(NftMinterTypeError::SupplyCap),
    )
}

pub fn mint_supply_cap_changes_case() -> NftMinterCase {
    mint_from_counter_case_with(
        "mint_supply_cap_changes",
        6,
        7,
        101,
        [6u8; 32],
        Some([6u8; 32]),
        MintExpected::Input(NftMinterTypeError::SupplyCap),
    )
}

pub fn mint_reaches_supply_cap_case() -> NftMinterCase {
    mint_from_counter_case_with(
        "mint_reaches_supply_cap",
        99,
        100,
        100,
        [99u8; 32],
        Some([99u8; 32]),
        MintExpected::Pass,
    )
}

pub fn mint_missing_nft_output_case() -> NftMinterCase {
    mint_from_counter_case_with(
        "mint_missing_nft_output",
        6,
        7,
        100,
        [6u8; 32],
        None,
        MintExpected::Input(NftMinterTypeError::InvalidMintedNft),
    )
}

pub fn mint_wrong_attributes_case() -> NftMinterCase {
    mint_from_counter_case_with(
        "mint_wrong_attributes",
        6,
        7,
        100,
        [6u8; 32],
        Some([7u8; 32]),
        MintExpected::Input(NftMinterTypeError::InvalidMintedNft),
    )
}

pub fn mint_duplicate_nft_output_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let always_success_code = deploy_always_success_code(fixture.context_mut());
    let minter_type_code = deploy_nft_minter_type_code(fixture.context_mut());
    let lock = build_always_success_script(
        fixture.context_mut(),
        &always_success_code,
        b"owner".to_vec(),
    );
    let minter_script =
        build_nft_minter_type_script(fixture.context_mut(), &minter_type_code, [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_script.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_script = build_always_success_script(
        fixture.context_mut(),
        &always_success_code,
        nft_id(minter_hash, serial).to_vec(),
    );
    let minter_cell = typed_output(
        lock.script.clone(),
        minter_script.script.clone(),
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
    shape.push_prefix_cell_dep(minter_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_script.cell_dep.clone());
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    shape.push_remainder_output(minted_nft_output(
        &lock.script,
        &nft_script.script,
        minter_hash,
        serial,
        seed,
    ));
    shape.push_remainder_output(minted_nft_output(
        &lock.script,
        &nft_script.script,
        minter_hash,
        serial,
        seed,
    ));
    shape.tx_level_message(
        CobuildMessageBuilder::new()
            .input_type_action(minter_hash)
            .action_data(mint_nft_action_data(seed, script_hash(&lock.script)))
            .build(),
    );
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "mint_duplicate_nft_output",
        fixture,
        built,
        expected: NftMinterExpected::MinterInputType {
            input: minter_input,
            error: NftMinterTypeError::InvalidMintedNft,
        },
    }
}

pub fn mint_wrong_serial_case() -> NftMinterCase {
    mint_wrong_nft_data_case(
        "mint_wrong_serial",
        MintedNftDataOverride {
            serial: Some(7),
            rarity: Some(rarity_for_serial(6)),
            ..Default::default()
        },
    )
}

pub fn mint_wrong_rarity_case() -> NftMinterCase {
    mint_wrong_nft_data_case(
        "mint_wrong_rarity",
        MintedNftDataOverride {
            serial: Some(6),
            rarity: Some(2),
            ..Default::default()
        },
    )
}

pub fn mint_wrong_minter_hash_case() -> NftMinterCase {
    mint_wrong_nft_data_case(
        "mint_wrong_minter_hash",
        MintedNftDataOverride {
            minter_type_hash: Some([9; 32]),
            serial: Some(6),
            rarity: Some(rarity_for_serial(6)),
            ..Default::default()
        },
    )
}

#[derive(Clone, Copy, Debug, Default)]
struct MintedNftDataOverride {
    minter_type_hash: Option<[u8; 32]>,
    serial: Option<u64>,
    rarity: Option<u8>,
    attributes_hash: Option<[u8; 32]>,
}

fn mint_wrong_nft_data_case(
    name: &'static str,
    override_data: MintedNftDataOverride,
) -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let always_success_code = deploy_always_success_code(fixture.context_mut());
    let minter_type_code = deploy_nft_minter_type_code(fixture.context_mut());
    let lock = build_always_success_script(
        fixture.context_mut(),
        &always_success_code,
        b"owner".to_vec(),
    );
    let minter_script =
        build_nft_minter_type_script(fixture.context_mut(), &minter_type_code, [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_script.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_script = build_always_success_script(
        fixture.context_mut(),
        &always_success_code,
        nft_id(minter_hash, serial).to_vec(),
    );
    let minter_cell = typed_output(
        lock.script.clone(),
        minter_script.script.clone(),
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
    let data_serial = override_data.serial.unwrap_or(serial);
    let rarity = override_data
        .rarity
        .unwrap_or_else(|| rarity_for_serial(data_serial));
    let data_minter_hash = override_data.minter_type_hash.unwrap_or(minter_hash);
    let attributes_hash = override_data
        .attributes_hash
        .unwrap_or_else(|| attributes_hash(data_minter_hash, data_serial, rarity, seed));

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_script.cell_dep.clone());
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    shape.push_remainder_output(TestCellOutput::new(
        typed_output(
            lock.script.clone(),
            nft_script.script.clone(),
            200_000_000_000,
        ),
        minted_nft_data(MintedNftData {
            minter_type_hash: data_minter_hash,
            serial: data_serial,
            rarity,
            attributes_hash,
        }),
    ));
    shape.tx_level_message(
        CobuildMessageBuilder::new()
            .input_type_action(minter_hash)
            .action_data(mint_nft_action_data(seed, script_hash(&lock.script)))
            .build(),
    );
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name,
        fixture,
        built,
        expected: NftMinterExpected::MinterInputType {
            input: minter_input,
            error: NftMinterTypeError::InvalidMintedNft,
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MintExpected {
    Pass,
    Input(NftMinterTypeError),
}

fn mint_from_counter_case_with(
    name: &'static str,
    old_counter: u64,
    new_counter: u64,
    output_supply_cap: u64,
    seed: [u8; 32],
    nft_seed: Option<[u8; 32]>,
    expected: MintExpected,
) -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let always_success_code = deploy_always_success_code(fixture.context_mut());
    let minter_type_code = deploy_nft_minter_type_code(fixture.context_mut());
    let minted_nft_type_code = deploy_minted_nft_type_code(fixture.context_mut());
    let lock = build_always_success_script(
        fixture.context_mut(),
        &always_success_code,
        b"owner".to_vec(),
    );
    let minter_script =
        build_nft_minter_type_script(fixture.context_mut(), &minter_type_code, [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_script.script);
    let serial = old_counter;
    let rarity = rarity_for_serial(serial);
    let nft_id = nft_id(minter_hash, serial);
    let nft_script = nft_seed.map(|_| {
        if expected == MintExpected::Pass {
            build_minted_nft_type_script(fixture.context_mut(), &minted_nft_type_code, nft_id)
        } else {
            build_always_success_script(
                fixture.context_mut(),
                &always_success_code,
                nft_id.to_vec(),
            )
        }
    });
    let minter_cell = typed_output(
        lock.script.clone(),
        minter_script.script.clone(),
        200_000_000_000,
    );
    let minter_input = live_resolved_facts(
        fixture.context_mut(),
        minter_cell.clone(),
        minter_data(MinterState {
            mint_counter: old_counter,
            supply_cap: 100,
        }),
    );
    let minter_output = TestCellOutput::new(
        minter_cell,
        minter_data(MinterState {
            mint_counter: new_counter,
            supply_cap: output_supply_cap,
        }),
    );
    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_script.cell_dep.clone());
    if let Some(nft_script) = &nft_script {
        shape.push_prefix_cell_dep(nft_script.cell_dep.clone());
    }
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    if let (Some(nft_script), Some(nft_seed)) = (&nft_script, nft_seed) {
        let nft_data = minted_nft_data(MintedNftData {
            minter_type_hash: minter_hash,
            serial,
            rarity,
            attributes_hash: attributes_hash(minter_hash, serial, rarity, nft_seed),
        });
        let nft_output = TestCellOutput::new(
            typed_output(
                lock.script.clone(),
                nft_script.script.clone(),
                200_000_000_000,
            ),
            nft_data,
        );
        shape.push_remainder_output(nft_output);
    }
    shape.tx_level_message(
        CobuildMessageBuilder::new()
            .input_type_action(minter_hash)
            .action_data(mint_nft_action_data(seed, script_hash(&lock.script)))
            .build(),
    );
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    let expected = match expected {
        MintExpected::Pass => NftMinterExpected::Pass,
        MintExpected::Input(error) => NftMinterExpected::MinterInputType {
            input: minter_input,
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

pub fn mint_two_actions_tx_level_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let always_success_code = deploy_always_success_code(fixture.context_mut());
    let minter_type_code = deploy_nft_minter_type_code(fixture.context_mut());
    let minted_nft_type_code = deploy_minted_nft_type_code(fixture.context_mut());
    let lock = build_always_success_script(
        fixture.context_mut(),
        &always_success_code,
        b"owner".to_vec(),
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
    let minter_cell = typed_output(
        lock.script.clone(),
        minter_script.script.clone(),
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
    shape.push_prefix_cell_dep(minter_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_6_script.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_7_script.cell_dep.clone());
    shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    shape.push_remainder_output(minted_nft_output(
        &lock.script,
        &nft_6_script.script,
        minter_hash,
        6,
        [6u8; 32],
    ));
    shape.push_remainder_output(minted_nft_output(
        &lock.script,
        &nft_7_script.script,
        minter_hash,
        7,
        [7u8; 32],
    ));
    shape.tx_level_message(
        CobuildMessageBuilder::new()
            .push_action(
                ActionRole::InputType.into(),
                minter_hash,
                mint_nft_action_data([6u8; 32], script_hash(&lock.script)),
            )
            .push_action(
                ActionRole::InputType.into(),
                minter_hash,
                mint_nft_action_data([7u8; 32], script_hash(&lock.script)),
            )
            .build(),
    );
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "mint_two_actions_tx_level",
        fixture,
        built,
        expected: NftMinterExpected::Pass,
    }
}
