use tests::{
    fixtures::cobuild_otx_lock::{assert_coverage_manifest, cases, two_udt_transfer_otxs_case},
    framework::scenario::ExpectedOutcome,
};

#[test]
fn cobuild_otx_lock_cases_match_expected_outcomes() {
    let cases = cases();
    assert_coverage_manifest(&cases);

    for case in cases {
        case.expected.assert(&case.fixture, &case.built);
    }
}

#[test]
fn two_udt_transfer_otxs_have_distinct_lock_facts() {
    let case = two_udt_transfer_otxs_case(false);
    let facts = case.two_udt_transfer_facts.expect("two UDT transfer facts");

    assert_eq!(case.expected, ExpectedOutcome::Pass);
    assert_ne!(facts.otx_a_lock_hash, facts.otx_b_lock_hash);
}

#[test]
fn two_udt_transfer_otxs_with_fee_have_distinct_lock_facts() {
    let case = two_udt_transfer_otxs_case(true);
    let facts = case.two_udt_transfer_facts.expect("two UDT transfer facts");

    assert_eq!(case.expected, ExpectedOutcome::Pass);
    assert_ne!(facts.otx_a_lock_hash, facts.otx_b_lock_hash);
    assert_ne!(facts.fee_lock_hash, Some(facts.otx_a_lock_hash));
    assert_ne!(facts.fee_lock_hash, Some(facts.otx_b_lock_hash));
}
