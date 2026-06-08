use tests::fixtures::limit_order::{failed_txs_count, limit_order_case};

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
