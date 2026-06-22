use ckb_testtool::ckb_types::{bytes::Bytes, core::ScriptHashType, prelude::*};
use cobuild_types::entity::{
    core::{Message as CobuildMessage, Otx, SighashAll},
    witness::{WitnessLayout, WitnessLayoutUnion},
};

use crate::fixtures::common::{
    assets::{nft_data, udt_amount_data},
    contracts::{
        deploy_always_success, deploy_input_type_proxy_lock, deploy_test_nft, deploy_test_udt,
        deploy_wrong_owner_lock,
    },
};
use crate::framework::{
    cells::{TestCellOutput, live_resolved_facts, normal_output, typed_output},
    cobuild::{ActionRole, empty_message},
    contracts::cell_dep_for_script,
    fixture::CobuildTestFixture,
    scenario::{ExpectedOutcome, ScriptLocation},
    scripts::script_hash,
    tx::{BuiltTxShape, InputHandle, OtxHandle, OtxSegment, OutputHandle, TxShape, WitnessHandle},
};

use super::{
    ActionSourceKind, BuiltLimitOrderCase, BusinessMutation, CoverageTag, FlowKind,
    LimitOrderAction, LimitOrderExpectedOutcome, LimitOrderFixtureExt, LimitOrderHappyPath,
    LimitOrderState, LimitOrderTypeError, NFT_TYPE_ARGS, OFFERED_ASSET_ID, OtxScopeKind,
    REQUESTED_ASSET_ID, ScriptRoleKind, actions::encode_action, order_data, settlement_data,
};

mod create_order;
mod fill;
mod legacy_settlement;
mod real_otx_lock;

pub fn type_script_cases() -> Vec<BuiltLimitOrderCase> {
    let mut cases = Vec::new();
    cases.extend(type_script_legacy_settlement_cases());
    cases.extend(type_script_fill_cases());
    cases.extend(real_otx_lock::real_otx_lock_cases());
    cases.extend(type_script_create_order_cases());
    cases
}

pub fn type_script_legacy_settlement_cases() -> Vec<BuiltLimitOrderCase> {
    legacy_settlement::type_script_legacy_settlement_cases()
}

pub fn type_script_fill_cases() -> Vec<BuiltLimitOrderCase> {
    fill::type_script_fill_cases()
}

pub fn type_script_create_order_cases() -> Vec<BuiltLimitOrderCase> {
    create_order::type_script_create_order_cases()
}

fn limit_order_input(
    fixture: &mut CobuildTestFixture,
    owner: ckb_testtool::ckb_types::packed::Script,
    offered_nft_type_hash: [u8; 32],
    requested_asset_id: [u8; 32],
    requested_amount: u64,
    limit_order_type: &ckb_testtool::ckb_types::packed::Script,
) -> crate::framework::cells::ResolvedInputFacts {
    let owner_lock_hash = script_hash(&owner);
    live_resolved_facts(
        fixture.context_mut(),
        typed_output(owner, limit_order_type.clone(), 100_000_000_000),
        order_data(LimitOrderState {
            owner_lock_hash,
            offered_nft_type_hash,
            requested_asset_id,
            requested_amount,
        }),
    )
}

fn fill_message(
    fixture: &CobuildTestFixture,
    target_hash: [u8; 32],
    payment: OutputHandle,
    buyer_lock_hash: [u8; 32],
    built: &BuiltTxShape,
) -> CobuildMessage {
    let action = LimitOrderAction::Fill {
        payment,
        buyer_lock_hash,
    };
    fixture
        .cobuild()
        .input_type_action(target_hash)
        .action_data(encode_action(&action, built))
        .build()
}

fn fill_output_type_message(
    fixture: &CobuildTestFixture,
    target_hash: [u8; 32],
    payment: OutputHandle,
    buyer_lock_hash: [u8; 32],
    built: &BuiltTxShape,
) -> CobuildMessage {
    let action = LimitOrderAction::Fill {
        payment,
        buyer_lock_hash,
    };
    fixture
        .cobuild()
        .output_type_action(target_hash)
        .action_data(encode_action(&action, built))
        .build()
}

fn replace_otx_message(built: &mut BuiltTxShape, otx: OtxHandle, message: CobuildMessage) {
    let witness = built.otx_witness(otx);
    let tx_index = built.witnesses.tx_index(witness);
    let current = built
        .tx
        .witnesses()
        .into_iter()
        .nth(tx_index)
        .expect("OTX witness")
        .raw_data();
    let otx = match WitnessLayout::from_slice(current.as_ref())
        .expect("parse OTX witness")
        .to_enum()
    {
        WitnessLayoutUnion::Otx(otx) => otx,
        other => panic!("expected OTX witness, got {}", other.item_name()),
    };
    let updated = otx.as_builder().message(message).build();
    replace_witness_bytes(built, witness, otx_witness_bytes(updated));
}

fn replace_tx_level_message(built: &mut BuiltTxShape, message: CobuildMessage) {
    let tx_level_witness = built.tx_level_witness();
    let witness = WitnessLayout::from(
        SighashAll::new_builder()
            .seal(Vec::<u8>::new())
            .message(message)
            .build(),
    );
    replace_witness_bytes(
        built,
        tx_level_witness,
        Bytes::copy_from_slice(witness.as_slice()),
    );
}

fn replace_witness_bytes(built: &mut BuiltTxShape, witness: WitnessHandle, replacement: Bytes) {
    let tx_index = built.witnesses.tx_index(witness);
    let mut witnesses: Vec<_> = built.tx.witnesses().into_iter().collect();
    witnesses[tx_index] = replacement.pack();
    built.tx = built
        .tx
        .as_advanced_builder()
        .set_witnesses(witnesses)
        .build();
}

fn otx_witness_bytes(otx: Otx) -> Bytes {
    let witness = WitnessLayout::from(otx);
    Bytes::copy_from_slice(witness.as_slice())
}

fn input_type_error(input: InputHandle, error: LimitOrderTypeError) -> LimitOrderExpectedOutcome {
    LimitOrderExpectedOutcome::InputType { input, error }
}

fn output_type_error(
    output: OutputHandle,
    error: LimitOrderTypeError,
) -> LimitOrderExpectedOutcome {
    LimitOrderExpectedOutcome::OutputType { output, error }
}

fn built_case(
    name: impl Into<String>,
    fixture: CobuildTestFixture,
    built: BuiltTxShape,
    expected: LimitOrderExpectedOutcome,
    coverage: CoverageTag,
) -> BuiltLimitOrderCase {
    BuiltLimitOrderCase {
        name: name.into(),
        fixture,
        built,
        signing_facts: Vec::new(),
        expected,
        coverage: vec![coverage],
    }
}

fn coverage(
    flow: FlowKind,
    script_role: ScriptRoleKind,
    otx_scope: OtxScopeKind,
    action_source: super::ActionSourceKind,
    mutation: Option<BusinessMutation>,
) -> CoverageTag {
    let tag = CoverageTag::new(flow, script_role, otx_scope, action_source);
    if let Some(mutation) = mutation {
        tag.with_mutation(mutation)
    } else {
        tag
    }
}

fn push_deps<'a>(
    shape: &mut TxShape,
    scripts: impl IntoIterator<Item = &'a crate::framework::contracts::DeployedScript>,
) {
    for script in scripts {
        shape.push_prefix_cell_dep(cell_dep_for_script(script));
    }
}
