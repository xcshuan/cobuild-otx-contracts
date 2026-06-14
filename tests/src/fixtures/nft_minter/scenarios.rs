use ckb_hash::new_blake2b;
use ckb_testtool::ckb_types::{bytes::Bytes, packed::CellInput, prelude::*};

use crate::{
    fixtures::{
        common::contracts::{
            deploy_always_success, deploy_minted_nft_type, deploy_nft_minter_type,
            rebuild_data2_script,
        },
        nft_minter::{
            attributes_hash, create_minter_action_data, mint_nft_action_data, minted_nft_data,
            minter_data, nft_id, rarity_for_serial, MintedNftData, MinterState, NftMinterExpected,
        },
    },
    framework::{
        cells::{live_resolved_facts, normal_output, typed_output, TestCellOutput},
        cobuild::CobuildMessageBuilder,
        fixture::CobuildTestFixture,
        scripts::script_hash,
        tx::BuiltTxShape,
        tx::TxShape,
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
}

pub fn create_minter_case() -> NftMinterCase {
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
            mint_counter: 0,
            supply_cap: 10,
        }),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_prefix_input(funding_input);
    shape.push_remainder_output(output);
    shape.tx_level_message(
        CobuildMessageBuilder::new()
            .output_type_action(minter_hash)
            .action_data(create_minter_action_data(10))
            .build(),
    );
    let mut built = shape.build();
    built.tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "create_minter",
        fixture,
        built,
        expected: NftMinterExpected::Pass,
    }
}

pub fn mint_first_nft_case() -> NftMinterCase {
    mint_from_counter_case("mint_first_nft", 0, 1, [9u8; 32])
}

pub fn mint_from_counter_six_case() -> NftMinterCase {
    mint_from_counter_case("mint_from_counter_six", 6, 7, [6u8; 32])
}

fn mint_from_counter_case(
    name: &'static str,
    old_counter: u64,
    new_counter: u64,
    seed: [u8; 32],
) -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_code.script);
    let serial = old_counter;
    let rarity = rarity_for_serial(serial);
    let nft_id = nft_id(minter_hash, serial);
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), nft_id);
    let nft_data = minted_nft_data(MintedNftData {
        minter_type_hash: minter_hash,
        serial,
        rarity,
        attributes_hash: attributes_hash(minter_hash, serial, rarity, seed),
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
            supply_cap: 100,
        }),
    );
    let nft_output = TestCellOutput::new(
        typed_output(
            lock.script.clone(),
            nft_code.script.clone(),
            200_000_000_000,
        ),
        nft_data,
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock.cell_dep.clone());
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    shape.push_remainder_output(nft_output);
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
        expected: NftMinterExpected::Pass,
    }
}

fn type_id_args(first_input: &CellInput, output_index: u64) -> [u8; 32] {
    let mut blake2b = new_blake2b();
    blake2b.update(first_input.as_slice());
    blake2b.update(&output_index.to_le_bytes());
    let mut out = [0u8; 32];
    blake2b.finalize(&mut out);
    out
}
