use ckb_hash::new_blake2b;
use ckb_testtool::ckb_types::{bytes::Bytes, packed::CellInput, prelude::*};

use crate::{
    fixtures::{
        cobuild_otx_lock::CobuildOtxLockError,
        common::contracts::{
            deploy_always_success, deploy_cobuild_otx_lock, deploy_minted_nft_type,
            deploy_nft_minter_type, rebuild_data2_script,
        },
        nft_minter::{
            MintedNftData, MintedNftTypeError, MinterState, NftMinterExpected, NftMinterTypeError,
            attributes_hash, create_minter_action_data, mint_nft_action_data, minted_nft_data,
            minter_data, nft_id, rarity_for_serial,
        },
    },
    framework::{
        cells::{TestCellOutput, live_resolved_facts, normal_output, typed_output},
        cobuild::{ActionRole, CobuildMessageBuilder},
        fixture::CobuildTestFixture,
        scripts::script_hash,
        signing::{
            SignatureScope, SignerId, TestSigningHashOracle, fixed_secret_key, public_key_hash20,
            sign_scope,
        },
        tx::TxShape,
        tx::{BuiltTxShape, OtxSegment, ProtocolMutation, TxShapeMutation},
    },
};

pub struct NftMinterCase {
    pub name: &'static str,
    pub fixture: CobuildTestFixture,
    pub built: BuiltTxShape,
    pub expected: NftMinterExpected,
}

impl NftMinterCase {
    pub fn assert_expected(&self) {
        self.expected.assert(&self.fixture, &self.built);
    }

    pub fn assert_expected_with_context(&self) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.assert_expected();
        }));
        if let Err(payload) = result {
            std::panic::resume_unwind(Box::new(format!(
                "nft minter case `{}` failed: {}",
                self.name,
                panic_message(payload)
            )));
        }
    }
}

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
            .action_data(mint_nft_action_data(seed))
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
            .action_data(mint_nft_action_data(seed))
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
    let nft_code = nft_seed.map(|_| deploy_minted_nft_type(fixture.context_mut(), nft_id));
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
            .action_data(mint_nft_action_data(seed))
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
                mint_nft_action_data([6u8; 32]),
            )
            .push_action(
                ActionRole::InputType.into(),
                minter_hash,
                mint_nft_action_data([7u8; 32]),
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
            .action_data(mint_nft_action_data([6u8; 32]))
            .build(),
    );
    shape.push_otx(OtxSegment {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data([7u8; 32]))
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
                .action_data(mint_nft_action_data(seed))
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
                .action_data(mint_nft_action_data(seed))
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
                .action_data(mint_nft_action_data(seed))
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RealOtxLockMintMode {
    Valid,
    TamperBaseOutput,
    MissingBaseSeal,
    BadBaseSeal,
}

fn real_otx_lock_mint_case(name: &'static str, mode: RealOtxLockMintMode) -> NftMinterCase {
    let secret_key = fixed_secret_key(42);
    let mut fixture = CobuildTestFixture::new();
    let user_lock =
        deploy_cobuild_otx_lock(fixture.context_mut(), 0, &public_key_hash20(&secret_key));
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
    shape.push_prefix_cell_dep(user_lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    let otx = shape.push_otx(OtxSegment {
        message: Some(
            CobuildMessageBuilder::new()
                .input_type_action(minter_hash)
                .action_data(mint_nft_action_data(seed))
                .build(),
        ),
        base_inputs: vec![minter_input],
        base_outputs: vec![minter_output],
        append_outputs: vec![minted_output.clone()],
        ..Default::default()
    });
    let minter_input = shape.otx_base_input(otx, 0);
    let minter_base_output = shape.otx_base_output(otx, 0);
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

    let expected = match mode {
        RealOtxLockMintMode::Valid => NftMinterExpected::Pass,
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

fn minted_nft_output(
    lock_script: &ckb_testtool::ckb_types::packed::Script,
    nft_script: &ckb_testtool::ckb_types::packed::Script,
    minter_hash: [u8; 32],
    serial: u64,
    seed: [u8; 32],
) -> TestCellOutput {
    let rarity = rarity_for_serial(serial);
    TestCellOutput::new(
        typed_output(lock_script.clone(), nft_script.clone(), 200_000_000_000),
        minted_nft_data(MintedNftData {
            minter_type_hash: minter_hash,
            serial,
            rarity,
            attributes_hash: attributes_hash(minter_hash, serial, rarity, seed),
        }),
    )
}

fn type_id_args(first_input: &CellInput, output_index: u64) -> [u8; 32] {
    let mut blake2b = new_blake2b();
    blake2b.update(first_input.as_slice());
    blake2b.update(&output_index.to_le_bytes());
    let mut out = [0u8; 32];
    blake2b.finalize(&mut out);
    out
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_owned()
    } else {
        "non-string panic payload".to_owned()
    }
}
