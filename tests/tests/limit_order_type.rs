use tests::fixtures::limit_order::{failed_txs_count, type_script_cases};

fn assert_no_expected_failure_dump(before: usize) {
    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), before);
    }
}

#[test]
fn limit_order_type_script_cases_match_expected_outcomes() {
    for case in type_script_cases() {
        let before = failed_txs_count();

        case.assert_expected();
        assert!(
            !case.coverage.is_empty(),
            "limit order type case must declare coverage"
        );

        if !case.expected.is_pass() {
            assert_no_expected_failure_dump(before);
        }
    }
}
