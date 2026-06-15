use super::*;

pub fn forged_nft_creation_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_hash = [1u8; 32];
    let serial = 6;
    let seed = [6u8; 32];
    let rarity = rarity_for_serial(serial);
    let nft_id = nft_id(minter_hash, serial);
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), nft_id);
    let nft_output = TestCellOutput::new(
        typed_output(
            lock.script.clone(),
            nft_code.script.clone(),
            200_000_000_000,
        ),
        minted_nft_data(MintedNftData {
            minter_type_hash: minter_hash,
            serial,
            rarity,
            attributes_hash: attributes_hash(minter_hash, serial, rarity, seed),
        }),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    let nft_output = shape.push_remainder_output(nft_output);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "forged_nft_creation",
        fixture,
        built,
        expected: NftMinterExpected::MintedNftOutputType {
            output: nft_output,
            error: MintedNftTypeError::InvalidMinterTransition,
        },
    }
}

pub fn nft_valid_transfer_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_hash = [1u8; 32];
    let serial = 6;
    let seed = [6u8; 32];
    let rarity = rarity_for_serial(serial);
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), nft_id(minter_hash, serial));
    let nft_data = minted_nft_data(MintedNftData {
        minter_type_hash: minter_hash,
        serial,
        rarity,
        attributes_hash: attributes_hash(minter_hash, serial, rarity, seed),
    });
    let nft_cell = typed_output(
        lock.script.clone(),
        nft_code.script.clone(),
        200_000_000_000,
    );
    let nft_input = live_resolved_facts(fixture.context_mut(), nft_cell.clone(), nft_data.clone());
    let nft_output = TestCellOutput::new(nft_cell, nft_data);

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    shape.push_prefix_input(nft_input);
    shape.push_remainder_output(nft_output);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "nft_valid_transfer",
        fixture,
        built,
        expected: NftMinterExpected::Pass,
    }
}

pub fn nft_create_wrong_args_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_hash = [1u8; 32];
    let serial = 6;
    let seed = [6u8; 32];
    let rarity = rarity_for_serial(serial);
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), [9u8; 32]);
    let nft_output = TestCellOutput::new(
        typed_output(
            lock.script.clone(),
            nft_code.script.clone(),
            200_000_000_000,
        ),
        minted_nft_data(MintedNftData {
            minter_type_hash: minter_hash,
            serial,
            rarity,
            attributes_hash: attributes_hash(minter_hash, serial, rarity, seed),
        }),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    let nft_output = shape.push_remainder_output(nft_output);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "nft_create_wrong_args",
        fixture,
        built,
        expected: NftMinterExpected::MintedNftOutputType {
            output: nft_output,
            error: MintedNftTypeError::InvalidArgs,
        },
    }
}

pub fn nft_create_serial_outside_minter_transition_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_type = deploy_always_success(fixture.context_mut(), b"minter-type".to_vec());
    let minter_hash = script_hash(&minter_type.script);
    let serial = 5;
    let seed = [5u8; 32];
    let rarity = rarity_for_serial(serial);
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), nft_id(minter_hash, serial));
    let minter_cell = typed_output(
        lock.script.clone(),
        minter_type.script.clone(),
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
            mint_counter: 7,
            supply_cap: 100,
        }),
    );
    let nft_output = TestCellOutput::new(
        typed_output(
            lock.script.clone(),
            nft_code.script.clone(),
            200_000_000_000,
        ),
        minted_nft_data(MintedNftData {
            minter_type_hash: minter_hash,
            serial,
            rarity,
            attributes_hash: attributes_hash(minter_hash, serial, rarity, seed),
        }),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_type.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    let nft_output = shape.push_remainder_output(nft_output);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "nft_create_serial_outside_minter_transition",
        fixture,
        built,
        expected: NftMinterExpected::MintedNftOutputType {
            output: nft_output,
            error: MintedNftTypeError::InvalidMinterTransition,
        },
    }
}

pub fn nft_multiple_group_outputs_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_hash = [1u8; 32];
    let serial = 6;
    let seed = [6u8; 32];
    let rarity = rarity_for_serial(serial);
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), nft_id(minter_hash, serial));
    let nft_data = minted_nft_data(MintedNftData {
        minter_type_hash: minter_hash,
        serial,
        rarity,
        attributes_hash: attributes_hash(minter_hash, serial, rarity, seed),
    });
    let nft_cell = typed_output(
        lock.script.clone(),
        nft_code.script.clone(),
        200_000_000_000,
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    let first_output =
        shape.push_remainder_output(TestCellOutput::new(nft_cell.clone(), nft_data.clone()));
    shape.push_remainder_output(TestCellOutput::new(nft_cell, nft_data));
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "nft_multiple_group_outputs",
        fixture,
        built,
        expected: NftMinterExpected::MintedNftOutputType {
            output: first_output,
            error: MintedNftTypeError::InvalidShape,
        },
    }
}

pub fn nft_multiple_group_inputs_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_hash = [1u8; 32];
    let serial = 6;
    let seed = [6u8; 32];
    let rarity = rarity_for_serial(serial);
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), nft_id(minter_hash, serial));
    let nft_data = minted_nft_data(MintedNftData {
        minter_type_hash: minter_hash,
        serial,
        rarity,
        attributes_hash: attributes_hash(minter_hash, serial, rarity, seed),
    });
    let nft_cell = typed_output(
        lock.script.clone(),
        nft_code.script.clone(),
        200_000_000_000,
    );
    let first_input =
        live_resolved_facts(fixture.context_mut(), nft_cell.clone(), nft_data.clone());
    let second_input = live_resolved_facts(fixture.context_mut(), nft_cell, nft_data);

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    let first_input = shape.push_prefix_input(first_input);
    shape.push_prefix_input(second_input);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "nft_multiple_group_inputs",
        fixture,
        built,
        expected: NftMinterExpected::MintedNftInputType {
            input: first_input,
            error: MintedNftTypeError::InvalidShape,
        },
    }
}

pub fn nft_transfer_mutates_data_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_hash = [1u8; 32];
    let serial = 6;
    let seed = [6u8; 32];
    let rarity = rarity_for_serial(serial);
    let nft_id = nft_id(minter_hash, serial);
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), nft_id);
    let input_data = minted_nft_data(MintedNftData {
        minter_type_hash: minter_hash,
        serial,
        rarity,
        attributes_hash: attributes_hash(minter_hash, serial, rarity, seed),
    });
    let output_data = minted_nft_data(MintedNftData {
        minter_type_hash: minter_hash,
        serial,
        rarity,
        attributes_hash: attributes_hash(minter_hash, serial, rarity, [7u8; 32]),
    });
    let nft_cell = typed_output(
        lock.script.clone(),
        nft_code.script.clone(),
        200_000_000_000,
    );
    let nft_input = live_resolved_facts(fixture.context_mut(), nft_cell.clone(), input_data);
    let nft_output = TestCellOutput::new(nft_cell, output_data);

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    let nft_input = shape.push_prefix_input(nft_input);
    shape.push_remainder_output(nft_output);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "nft_transfer_mutates_data",
        fixture,
        built,
        expected: NftMinterExpected::MintedNftInputType {
            input: nft_input,
            error: MintedNftTypeError::InvalidNftData,
        },
    }
}

pub fn nft_burn_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_hash = [1u8; 32];
    let serial = 6;
    let seed = [6u8; 32];
    let rarity = rarity_for_serial(serial);
    let nft_id = nft_id(minter_hash, serial);
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), nft_id);
    let nft_cell = typed_output(
        lock.script.clone(),
        nft_code.script.clone(),
        200_000_000_000,
    );
    let nft_input = live_resolved_facts(
        fixture.context_mut(),
        nft_cell,
        minted_nft_data(MintedNftData {
            minter_type_hash: minter_hash,
            serial,
            rarity,
            attributes_hash: attributes_hash(minter_hash, serial, rarity, seed),
        }),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    shape.push_prefix_input(nft_input);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "nft_burn",
        fixture,
        built,
        expected: NftMinterExpected::Pass,
    }
}
