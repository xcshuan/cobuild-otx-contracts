use tests::fixtures::limit_order::{
    CreateOrderCase, FillActionCase, NftForUdtPaymentCase, failed_txs_count,
    limit_order_action_failure_case, limit_order_case, limit_order_create_nft_order_case,
    limit_order_create_nft_order_case_with, limit_order_nft_for_udt_case,
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
fn limit_order_type_accepts_create_order_with_nft_proxy_output() {
    let (fixture, tx) = limit_order_create_nft_order_case();

    fixture.assert_pass(&tx);
}

#[test]
fn limit_order_type_rejects_create_order_without_nft_proxy_output() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) =
        limit_order_create_nft_order_case_with(CreateOrderCase::MissingNftProxyOutput);

    fixture.assert_output_type_script_exit(&tx, 0, 12);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_create_order_wrong_nft_type() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_create_nft_order_case_with(CreateOrderCase::WrongNftType);

    fixture.assert_output_type_script_exit(&tx, 0, 12);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_create_order_wrong_proxy_order() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_create_nft_order_case_with(CreateOrderCase::WrongProxyOrder);

    fixture.assert_output_type_script_exit(&tx, 0, 12);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_create_order_state_action_mismatch() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) =
        limit_order_create_nft_order_case_with(CreateOrderCase::StateActionMismatch);

    fixture.assert_output_type_script_exit(&tx, 0, 10);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_create_order_invalid_type_id() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_create_nft_order_case_with(CreateOrderCase::InvalidTypeId);

    fixture.assert_output_type_script_exit(&tx, 0, 14);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_order_input_and_output_group_shape() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) =
        limit_order_create_nft_order_case_with(CreateOrderCase::InputAndOutputGroupShape);

    fixture.assert_type_script_exit(&tx, 0, 5);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
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

#[test]
fn limit_order_type_rejects_tx_level_fill_order() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::TxLevelFillOrder);

    fixture.assert_type_script_exit(&tx, 0, 12);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_output_type_fill_order_target() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::OutputTypeTarget);

    fixture.assert_type_script_exit(&tx, 0, 12);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_requested_asset_mismatch() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::RequestedAssetMismatch);

    fixture.assert_type_script_exit(&tx, 0, 10);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_fill_amount_below_order_minimum() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::MinRequestedBelowRequired);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_payment_in_another_otx() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::PaymentInAnotherOtx);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}
