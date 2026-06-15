use super::*;

pub fn minter_burn_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_input = live_resolved_facts(
        fixture.context_mut(),
        typed_output(
            lock.script.clone(),
            minter_code.script.clone(),
            200_000_000_000,
        ),
        minter_data(MinterState {
            mint_counter: 6,
            supply_cap: 100,
        }),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    let minter_input = shape.push_prefix_input(minter_input);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "minter_burn",
        fixture,
        built,
        expected: NftMinterExpected::MinterInputType {
            input: minter_input,
            error: NftMinterTypeError::InvalidShape,
        },
    }
}

pub fn minter_multiple_group_outputs_case() -> NftMinterCase {
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
    let output = TestCellOutput::new(
        typed_output(lock.script.clone(), minter_script, 200_000_000_000),
        minter_data(MinterState {
            mint_counter: 0,
            supply_cap: 100,
        }),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_prefix_input(funding_input);
    let first_output = shape.push_remainder_output(output.clone());
    shape.push_remainder_output(output);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "minter_multiple_group_outputs",
        fixture,
        built,
        expected: NftMinterExpected::MinterOutputType {
            output: first_output,
            error: NftMinterTypeError::InvalidShape,
        },
    }
}

pub fn minter_multiple_group_inputs_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_cell = typed_output(
        lock.script.clone(),
        minter_code.script.clone(),
        200_000_000_000,
    );
    let minter_data = minter_data(MinterState {
        mint_counter: 6,
        supply_cap: 100,
    });
    let first_input = live_resolved_facts(
        fixture.context_mut(),
        minter_cell.clone(),
        minter_data.clone(),
    );
    let second_input = live_resolved_facts(fixture.context_mut(), minter_cell, minter_data);

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    let first_input = shape.push_prefix_input(first_input);
    shape.push_prefix_input(second_input);
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "minter_multiple_group_inputs",
        fixture,
        built,
        expected: NftMinterExpected::MinterInputType {
            input: first_input,
            error: NftMinterTypeError::InvalidShape,
        },
    }
}
