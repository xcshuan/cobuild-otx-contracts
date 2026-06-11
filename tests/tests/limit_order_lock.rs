use tests::fixtures::limit_order::{
    LimitOrderLockFillCase, failed_txs_count, limit_order_lock_nft_for_udt_case,
    limit_order_lock_nft_for_udt_case_with, limit_order_lock_otx_with_sighash_all_fill_case,
    mixed_limit_order_type_lock_duplicate_payment_case,
};

fn assert_no_expected_failure_dump(before: usize) {
    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), before);
    }
}

#[test]
fn limit_order_lock_accepts_nft_for_udt_single_otx_append_fill() {
    let (fixture, tx) = limit_order_lock_nft_for_udt_case();

    fixture.assert_pass(&tx);
}

#[test]
fn limit_order_lock_accepts_nft_for_udt_otx_with_sighash_all_fill() {
    let (fixture, tx) = limit_order_lock_otx_with_sighash_all_fill_case();

    fixture.assert_pass(&tx);
}

#[test]
fn limit_order_lock_rejects_malformed_lock_args() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::MalformedArgs);
    fixture.assert_lock_script_exit(&tx, 0, 5);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_wrong_nft_type() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::WrongNftType);
    fixture.assert_lock_script_exit(&tx, 0, 8);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_tx_level_fill_order() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::TxLevelFillOrder);
    fixture.assert_lock_script_exit(&tx, 0, 12);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_wrong_action_target() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::WrongActionTarget);
    fixture.assert_lock_script_exit(&tx, 0, 12);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_append_scope_input() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::OrderInputInAppendScope);
    fixture.assert_lock_script_exit(&tx, 1, 12);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_insufficient_udt() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::InsufficientUdt);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_wrong_udt() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::WrongUdt);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_wrong_owner() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::WrongOwner);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_does_not_count_tx_level_remainder_payment() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::TxLevelRemainderOnly);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_does_not_count_payment_in_another_otx() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::PaymentInAnotherOtx);
    fixture.assert_lock_script_exit(&tx, 0, 12);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_payment_output_outside_current_otx() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::PaymentOutputOutOfRange);
    fixture.assert_lock_script_exit(&tx, 0, 12);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_bound_payment_output_wrong_udt() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::PaymentOutputWrongUdt);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_bound_payment_output_wrong_owner() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::PaymentOutputWrongOwner);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_bound_payment_output_insufficient() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::PaymentOutputInsufficient);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn fill_lock_order_rejects_missing_buyer_nft_output() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::MissingBuyerNftOutput);
    fixture.assert_lock_script_exit(&tx, 0, 12);
    assert_no_expected_failure_dump(before);
}

#[test]
fn fill_lock_order_rejects_buyer_nft_output_with_wrong_lock() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::BuyerNftWrongLock);
    fixture.assert_lock_script_exit(&tx, 0, 12);
    assert_no_expected_failure_dump(before);
}

#[test]
fn fill_lock_order_rejects_buyer_nft_output_with_wrong_type() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::BuyerNftWrongType);
    fixture.assert_lock_script_exit(&tx, 0, 12);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_unknown_action_tag() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::UnknownActionTag);
    fixture.assert_lock_script_exit(&tx, 0, 7);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_malformed_action_payload() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::MalformedAction);
    fixture.assert_lock_script_exit(&tx, 0, 6);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_two_lock_orders_reusing_payment_output() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(
        LimitOrderLockFillCase::TwoLockOrdersReusePaymentOutput,
    );
    // The second order is the one that reuses the first order's payment output.
    fixture.assert_lock_script_exit(&tx, 1, 12);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_accepts_two_lock_orders_with_distinct_payment_outputs() {
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(
        LimitOrderLockFillCase::TwoLockOrdersUseDistinctPaymentOutputs,
    );
    fixture.assert_pass(&tx);
}

#[test]
fn limit_order_mixed_type_and_lock_reject_duplicate_payment_output() {
    let before = failed_txs_count();
    let (fixture, tx) = mixed_limit_order_type_lock_duplicate_payment_case();
    fixture.assert_lock_script_exit(&tx, 2, 12);
    assert_no_expected_failure_dump(before);
}
