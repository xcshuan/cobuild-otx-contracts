use ckb_testtool::ckb_types::core::TransactionView;
use tests::{
    fixtures::limit_order::{LimitOrderBuilder, LimitOrderCobuildMessageExt, LimitOrderFixtureExt},
    framework::{contracts::cell_dep_for_script, fixture::CobuildTestFixture},
};

const ORDER_ID: [u8; 32] = [1; 32];
const OFFERED_ASSET_ID: [u8; 32] = [3; 32];
const REQUESTED_ASSET_ID: [u8; 32] = [4; 32];

#[test]
fn limit_order_accepts_otx_append_settlement_at_limit_price() {
    let (fixture, tx) = limit_order_case(30);

    fixture.assert_pass(&tx);
}

#[test]
fn limit_order_rejects_otx_append_settlement_below_limit_price() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_case(29);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

fn limit_order_case(settlement_amount: u64) -> (CobuildTestFixture, TransactionView) {
    let mut fixture = CobuildTestFixture::new();

    let limit_order = fixture.deploy_limit_order();
    let always_success = fixture.deploy_always_success();
    let owner_lock = always_success.script.clone();

    let order_input = fixture
        .limit_order()
        .owner(owner_lock.clone())
        .offered_asset_id(OFFERED_ASSET_ID)
        .requested_asset_id(REQUESTED_ASSET_ID)
        .offered_remaining(10)
        .min_requested_per_offered(3)
        .build_input(&limit_order.script);

    let settlement_output = LimitOrderBuilder::settlement_output(
        owner_lock,
        REQUESTED_ASSET_ID,
        settlement_amount,
        90_000_000_000,
    );

    let message = fixture
        .cobuild()
        .input_type_action(limit_order.script_hash)
        .limit_order_fill(ORDER_ID, REQUESTED_ASSET_ID, 10, 30)
        .build();
    let otx = fixture
        .limit_order_append_settlement_otx()
        .message(message)
        .build_with_layout();

    let tx = fixture
        .tx()
        .cell_dep(cell_dep_for_script(&limit_order))
        .cell_dep(cell_dep_for_script(&always_success))
        .base_input(order_input)
        .append_output(settlement_output)
        .otx(otx)
        .build();

    (fixture, tx)
}

fn failed_txs_count() -> usize {
    let path = std::env::current_dir()
        .expect("current dir")
        .join("failed_txs");
    match std::fs::read_dir(path) {
        Ok(entries) => entries.count(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => panic!("read failed_txs: {error}"),
    }
}
