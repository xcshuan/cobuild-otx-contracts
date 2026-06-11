use tests::fixtures::limit_order::{failed_txs_count, type_script_cases};

fn assert_no_expected_failure_dump(before: usize) {
    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), before);
    }
}

#[test]
fn limit_order_type_script_cases_match_expected_outcomes() {
    let cases = type_script_cases();
    assert_eq!(cases.len(), 29, "limit order type case coverage count");
    assert!(
        cases
            .iter()
            .any(|case| case.name == "fill::NftForUdtScenario { payment_case: Valid, action_case: Some(TxLevelAndOtxFillOrder), sighash_all: false }"),
        "limit order type coverage must include duplicate tx-level plus OTX fill"
    );
    assert!(
        cases
            .iter()
            .any(|case| case.name == "fill::NftForUdtScenario { payment_case: Valid, action_case: Some(TxLevelNoiseAndOtxFillOrder), sighash_all: false }"),
        "limit order type coverage must include unrelated tx-level action noise plus OTX fill"
    );

    for case in cases {
        let before = failed_txs_count();

        case.assert_expected_with_context();
        assert!(
            !case.coverage.is_empty(),
            "limit order type case {} must declare coverage",
            case.name
        );

        if !case.expected.is_pass() {
            assert_no_expected_failure_dump(before);
        }
    }
}
