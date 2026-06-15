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
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_code.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_code =
        deploy_always_success(fixture.context_mut(), nft_id(minter_hash, serial).to_vec());
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
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    shape.push_remainder_output(minted_nft_output(
        &lock.script,
        &nft_code.script,
        minter_hash,
        serial,
        seed,
    ));
    shape.push_remainder_output(minted_nft_output(
        &lock.script,
        &nft_code.script,
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
        MintedNftData {
            minter_type_hash: [0; 32],
            serial: 7,
            rarity: rarity_for_serial(6),
            attributes_hash: [0; 32],
        },
    )
}

pub fn mint_wrong_rarity_case() -> NftMinterCase {
    mint_wrong_nft_data_case(
        "mint_wrong_rarity",
        MintedNftData {
            minter_type_hash: [0; 32],
            serial: 6,
            rarity: 2,
            attributes_hash: [0; 32],
        },
    )
}

pub fn mint_wrong_minter_hash_case() -> NftMinterCase {
    mint_wrong_nft_data_case(
        "mint_wrong_minter_hash",
        MintedNftData {
            minter_type_hash: [9; 32],
            serial: 6,
            rarity: rarity_for_serial(6),
            attributes_hash: [0; 32],
        },
    )
}

fn mint_wrong_nft_data_case(name: &'static str, override_data: MintedNftData) -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_code.script);
    let serial = 6;
    let seed = [6u8; 32];
    let nft_code =
        deploy_always_success(fixture.context_mut(), nft_id(minter_hash, serial).to_vec());
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
    let rarity = if override_data.rarity == 0 {
        rarity_for_serial(serial)
    } else {
        override_data.rarity
    };
    let data_minter_hash = if override_data.minter_type_hash == [0; 32] {
        minter_hash
    } else {
        override_data.minter_type_hash
    };
    let attributes_hash = if override_data.attributes_hash == [0; 32] {
        attributes_hash(data_minter_hash, override_data.serial, rarity, seed)
    } else {
        override_data.attributes_hash
    };

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    shape.push_remainder_output(TestCellOutput::new(
        typed_output(
            lock.script.clone(),
            nft_code.script.clone(),
            200_000_000_000,
        ),
        minted_nft_data(MintedNftData {
            minter_type_hash: data_minter_hash,
            serial: override_data.serial,
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
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_code.script);
    let serial = old_counter;
    let rarity = rarity_for_serial(serial);
    let nft_id = nft_id(minter_hash, serial);
    let nft_code = nft_seed.map(|_| {
        if expected == MintExpected::Pass {
            deploy_minted_nft_type(fixture.context_mut(), nft_id)
        } else {
            deploy_always_success(fixture.context_mut(), nft_id.to_vec())
        }
    });
    let minter_cell = typed_output(
        lock.script.clone(),
        minter_code.script.clone(),
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
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    if let Some(nft_code) = &nft_code {
        shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    }
    let minter_input = shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    if let (Some(nft_code), Some(nft_seed)) = (&nft_code, nft_seed) {
        let nft_data = minted_nft_data(MintedNftData {
            minter_type_hash: minter_hash,
            serial,
            rarity,
            attributes_hash: attributes_hash(minter_hash, serial, rarity, nft_seed),
        });
        let nft_output = TestCellOutput::new(
            typed_output(
                lock.script.clone(),
                nft_code.script.clone(),
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
    shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    shape.push_remainder_output(minted_nft_output(
        &lock.script,
        &nft_6_code.script,
        minter_hash,
        6,
        [6u8; 32],
    ));
    shape.push_remainder_output(minted_nft_output(
        &lock.script,
        &nft_7_code.script,
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
