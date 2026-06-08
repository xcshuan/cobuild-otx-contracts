use tests::fixtures::limit_order::{
    NftForUdtPaymentCase, failed_txs_count, limit_order_case, limit_order_nft_for_udt_case,
    limit_order_nft_for_udt_case_with,
};

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

#[test]
fn limit_order_type_accepts_nft_for_udt_otx_fill() {
    let (fixture, tx) = limit_order_nft_for_udt_case();

    fixture.assert_pass(&tx);
}

#[test]
fn limit_order_type_rejects_nft_for_udt_insufficient_udt() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_nft_for_udt_case_with(NftForUdtPaymentCase::InsufficientUdt);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_nft_for_udt_wrong_udt() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_nft_for_udt_case_with(NftForUdtPaymentCase::WrongUdt);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_nft_for_udt_wrong_owner() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_nft_for_udt_case_with(NftForUdtPaymentCase::WrongOwner);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_does_not_count_tx_level_remainder_udt() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) =
        limit_order_nft_for_udt_case_with(NftForUdtPaymentCase::TxLevelRemainderOnly);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}
