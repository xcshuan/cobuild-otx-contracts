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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CreateExpected {
    Pass,
    Output(NftMinterTypeError),
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
