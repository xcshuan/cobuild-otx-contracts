use ckb_testtool::ckb_types::{
    bytes::Bytes,
    core::{ScriptHashType, TransactionView},
};

use crate::framework::{
    cells::{TestCellOutput, live_input, typed_output},
    cobuild::seal_pair,
    contracts::{DeployedScript, cell_dep_for_script, deploy_data2_script},
    fixture::CobuildTestFixture,
    scripts::script_hash,
};

use super::{LimitOrderCobuildMessageExt, NFT_TYPE_ARGS};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitOrderLockFillCase {
    Valid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LockOrder {
    owner_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
    requested_asset_id: [u8; 32],
    min_requested_amount: u64,
}

pub fn limit_order_lock_nft_for_udt_case() -> (CobuildTestFixture, TransactionView) {
    limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::Valid)
}

pub fn limit_order_lock_nft_for_udt_case_with(
    case: LimitOrderLockFillCase,
) -> (CobuildTestFixture, TransactionView) {
    assert_eq!(case, LimitOrderLockFillCase::Valid);

    let mut fixture = CobuildTestFixture::new();
    let limit_order_lock_code =
        deploy_data2_script(fixture.context_mut(), "limit-order-lock", Vec::new());
    let always_success = fixture.deploy_always_success();
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let nft = deploy_test_nft(&mut fixture, NFT_TYPE_ARGS);
    let udt = deploy_test_udt_with_owner(&mut fixture, issuer_lock_hash);

    let order = LockOrder {
        owner_lock_hash: script_hash(&owner_lock),
        offered_nft_type_hash: nft.script_hash,
        requested_asset_id: udt.script_hash,
        min_requested_amount: 30,
    };
    let order_lock = fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_lock_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&lock_args(order)),
        )
        .expect("build limit order lock");
    let order_lock_hash = script_hash(&order_lock);

    let nft_payload = nft_data(b"lock-order-nft", [1, 2, 3, 4], 1_717_171_717);
    let nft_input = live_input(
        fixture.context_mut(),
        typed_output(order_lock.clone(), nft.script.clone(), 100_000_000_000),
        nft_payload.clone(),
    );
    let udt_input = live_input(
        fixture.context_mut(),
        typed_output(buyer_lock.clone(), udt.script.clone(), 100_000_000_000),
        udt_amount_data(30),
    );
    let nft_output = TestCellOutput::new(
        typed_output(buyer_lock, nft.script.clone(), 90_000_000_000),
        nft_payload,
    );
    let udt_payment_output = TestCellOutput::new(
        typed_output(owner_lock, udt.script.clone(), 90_000_000_000),
        udt_amount_data(30),
    );

    let message = fixture
        .cobuild()
        .input_lock_action(order_lock_hash)
        .limit_order_fill(udt.script_hash, 30)
        .build();
    let otx = fixture
        .otx()
        .base_input_cells(1)
        .base_output_cells(1)
        .append_input_cells(1)
        .append_output_cells(1)
        .allow_append_inputs()
        .allow_append_outputs()
        .message(message)
        .seals(vec![seal_pair(order_lock_hash, 0, Vec::new())])
        .build_with_layout();
    let tx = fixture
        .tx()
        .cell_dep(cell_dep_for_script(&limit_order_lock_code))
        .cell_dep(cell_dep_for_script(&always_success))
        .cell_dep(cell_dep_for_script(&nft))
        .cell_dep(cell_dep_for_script(&udt))
        .base_input(nft_input)
        .append_input(udt_input)
        .base_output(nft_output)
        .append_output(udt_payment_output)
        .otx(otx)
        .build();

    (fixture, tx)
}

fn lock_args(order: LockOrder) -> Vec<u8> {
    let mut data = Vec::with_capacity(104);
    data.extend_from_slice(&order.owner_lock_hash);
    data.extend_from_slice(&order.offered_nft_type_hash);
    data.extend_from_slice(&order.requested_asset_id);
    data.extend_from_slice(&order.min_requested_amount.to_le_bytes());
    data
}

fn deploy_test_udt_with_owner(
    fixture: &mut CobuildTestFixture,
    owner_lock_hash: [u8; 32],
) -> DeployedScript {
    deploy_data2_script(fixture.context_mut(), "test-udt", owner_lock_hash.to_vec())
}

fn deploy_test_nft(fixture: &mut CobuildTestFixture, args: [u8; 32]) -> DeployedScript {
    deploy_data2_script(fixture.context_mut(), "test-nft", args.to_vec())
}

fn nft_data(name: &[u8], attributes: [u8; 4], created_at: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + name.len() + 4 + 8);
    data.push(name.len() as u8);
    data.extend_from_slice(name);
    data.extend_from_slice(&attributes);
    data.extend_from_slice(&created_at.to_le_bytes());
    data
}

fn udt_amount_data(amount: u128) -> Vec<u8> {
    amount.to_le_bytes().to_vec()
}
