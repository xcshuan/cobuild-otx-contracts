use ckb_testtool::ckb_types::{bytes::Bytes, core::ScriptHashType, packed::Script, prelude::*};
use cobuild_types::entity::{
    core::{LockSeal, Message as CobuildMessage, Otx, SighashAll},
    witness::{WitnessLayout, WitnessLayoutUnion},
};

use crate::fixtures::common::{
    assets::{nft_data, udt_amount_data},
    contracts::{
        build_always_success_script, build_input_type_proxy_lock_script, build_test_nft_script,
        build_test_udt_script, build_wrong_owner_lock, deploy_always_success_code,
        deploy_input_type_proxy_lock_code, deploy_limit_order_lock, deploy_test_nft_code,
        deploy_test_udt_code,
    },
};
use crate::framework::{
    cells::{ResolvedInputFacts, TestCellOutput, live_resolved_facts, normal_output, typed_output},
    cobuild::{ActionRole, empty_message, lock_seal},
    contracts::{DeployedScript, cell_dep_for_script},
    fixture::CobuildTestFixture,
    scripts::script_hash,
    signing::{SignatureScope, SignerId, SigningFacts, SigningHashOracle, TestSigningHashOracle},
    tx::{
        BuiltTxShape, InputHandle, OtxHandle, OtxSpec, TxShape, WitnessHandle, append_segment_spec,
    },
};

use super::{
    ActionSourceKind, BuiltLimitOrderCase, BusinessMutation, CoverageTag, FlowKind,
    LimitOrderAction, LimitOrderExpectedOutcome, LimitOrderFixtureExt, LimitOrderHappyPath,
    LimitOrderLockError, LimitOrderState, NFT_TYPE_ARGS, OtxScopeKind, ScriptRoleKind,
    actions::encode_action, order_data,
};

mod fill;
mod mixed_type_lock;
mod multi_orders;
mod real_otx_lock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LockOrder {
    owner_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
    requested_asset_id: [u8; 32],
    requested_amount: u64,
}

pub fn lock_script_cases() -> Vec<BuiltLimitOrderCase> {
    let mut cases = Vec::new();
    cases.extend(lock_script_fill_cases());
    cases.extend(real_otx_lock::real_otx_lock_cases());
    cases.extend(mixed_type_lock_cases());
    cases
}

pub fn lock_script_fill_cases() -> Vec<BuiltLimitOrderCase> {
    let mut cases = fill::lock_script_fill_cases();
    cases.extend(multi_orders::multi_order_cases());
    cases
}

pub fn mixed_type_lock_cases() -> Vec<BuiltLimitOrderCase> {
    mixed_type_lock::mixed_type_lock_cases()
}

fn lock_script(
    fixture: &mut CobuildTestFixture,
    limit_order_lock_code: &DeployedScript,
    order: LockOrder,
    malformed_args: bool,
) -> Script {
    let mut args = lock_args(order);
    if malformed_args {
        args.pop();
    }
    fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_lock_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&args),
        )
        .expect("build limit order lock")
}

fn limit_order_type_input(
    fixture: &mut CobuildTestFixture,
    owner: Script,
    offered_nft_type_hash: [u8; 32],
    requested_asset_id: [u8; 32],
    requested_amount: u64,
    limit_order_type: &Script,
) -> ResolvedInputFacts {
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
    action: LimitOrderAction,
    built: &BuiltTxShape,
) -> CobuildMessage {
    fixture
        .cobuild()
        .input_lock_action(target_hash)
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

fn empty_lock_seal(script_hash: [u8; 32]) -> LockSeal {
    lock_seal(script_hash, Vec::new())
}

fn empty_seal_facts(
    built: &BuiltTxShape,
    script_hash: [u8; 32],
    scope: SignatureScope,
) -> SigningFacts {
    let oracle = TestSigningHashOracle;
    let signing_hash = match scope {
        SignatureScope::TxWithoutMessage | SignatureScope::TxWithMessage => {
            panic!("limit-order lock empty seals are OTX-scoped")
        }
        SignatureScope::OtxBase { otx } => oracle.otx_base(built, otx),
        SignatureScope::OtxAppendSegment { otx, segment_index } => {
            let base_hash = oracle.otx_base(built, otx);
            oracle.otx_append_segment(built, otx, segment_index, base_hash)
        }
    };
    SigningFacts {
        signer: SignerId("limit_order_lock_empty_seal"),
        scope,
        carrier: seal_carrier(built, scope),
        script_hash,
        signing_hash,
        seal: Vec::new(),
    }
}

fn seal_carrier(built: &BuiltTxShape, scope: SignatureScope) -> WitnessHandle {
    match scope {
        SignatureScope::OtxBase { otx } | SignatureScope::OtxAppendSegment { otx, .. } => {
            built.otx_witness(otx)
        }
        SignatureScope::TxWithoutMessage | SignatureScope::TxWithMessage => {
            panic!("limit-order lock empty seals are OTX-scoped")
        }
    }
}

fn input_lock_error(input: InputHandle, error: LimitOrderLockError) -> LimitOrderExpectedOutcome {
    LimitOrderExpectedOutcome::InputLock { input, error }
}

fn built_case(
    name: impl Into<String>,
    fixture: CobuildTestFixture,
    built: BuiltTxShape,
    signing_facts: Vec<SigningFacts>,
    expected: LimitOrderExpectedOutcome,
    coverage: CoverageTag,
) -> BuiltLimitOrderCase {
    BuiltLimitOrderCase {
        name: name.into(),
        fixture,
        built,
        signing_facts,
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

fn push_deps<'a>(shape: &mut TxShape, deps: impl IntoIterator<Item = &'a DeployedScript>) {
    for dep in deps {
        shape.push_prefix_cell_dep(cell_dep_for_script(dep));
    }
}

fn lock_args(order: LockOrder) -> Vec<u8> {
    let mut data = Vec::with_capacity(104);
    data.extend_from_slice(&order.owner_lock_hash);
    data.extend_from_slice(&order.offered_nft_type_hash);
    data.extend_from_slice(&order.requested_asset_id);
    data.extend_from_slice(&order.requested_amount.to_le_bytes());
    data
}
