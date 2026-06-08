use tests::{
    fixtures::{self, udt::two_udt_transfer_otxs_case},
    framework::assertions::assert_lock_script_exit_result,
};

#[test]
fn contract_rejects_invalid_args() {
    let result = fixtures::invalid_args_case().verify();
    assert_lock_script_exit_result(result, 0, 20);
}

#[test]
fn contract_rejects_without_relevant_signature_request() {
    let result = fixtures::no_relevant_signature_request_case().verify();
    assert_lock_script_exit_result(result, 0, 40);
}

#[test]
fn contract_accepts_sighash_all_cobuild_signature() {
    let result = fixtures::signed_sighash_all_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_sighash_all_when_current_lock_starts_after_input_zero() {
    let result = fixtures::signed_sighash_all_offset_lock_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_otx_base_and_append_signatures() {
    let result = fixtures::signed_otx_dual_scope_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_otx_signatures_covering_full_preimage_shape() {
    let result = fixtures::signed_otx_full_preimage_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_two_udt_transfer_otxs_in_one_transaction() {
    let case = two_udt_transfer_otxs_case(false);
    assert_ne!(case.otx_a_lock_hash, case.otx_b_lock_hash);

    let result = case.context.verify_tx(&case.tx, 50_000_000);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_two_udt_transfer_otxs_with_sighash_all_fee_input() {
    let case = two_udt_transfer_otxs_case(true);
    assert_ne!(case.otx_a_lock_hash, case.otx_b_lock_hash);
    assert_ne!(case.fee_lock_hash, Some(case.otx_a_lock_hash));
    assert_ne!(case.fee_lock_hash, Some(case.otx_b_lock_hash));

    let result = case.context.verify_tx(&case.tx, 50_000_000);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_mixed_sighash_all_and_otx_signature_requests() {
    let result = fixtures::mixed_sighash_all_and_otx_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_rejects_bad_seal() {
    let result = fixtures::bad_seal_case().verify();
    assert_lock_script_exit_result(result, 0, 50);
}

#[test]
fn contract_rejects_malformed_cobuild_witness() {
    let result = fixtures::malformed_cobuild_witness_case().verify();
    assert_lock_script_exit_result(result, 0, 30);
}

#[test]
fn contract_rejects_malformed_otx_layout() {
    let result = fixtures::malformed_otx_layout_case().verify();
    assert_lock_script_exit_result(result, 0, 31);
}
