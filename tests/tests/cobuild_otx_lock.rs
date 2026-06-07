use tests::fixtures;

#[test]
fn contract_rejects_invalid_args() {
    let result = fixtures::invalid_args_case().verify();
    assert_lock_script_exit(result, 20);
}

#[test]
fn contract_rejects_without_relevant_signature_request() {
    let result = fixtures::no_relevant_signature_request_case().verify();
    assert_lock_script_exit(result, 40);
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
fn contract_accepts_mixed_sighash_all_and_otx_signature_requests() {
    let result = fixtures::mixed_sighash_all_and_otx_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_rejects_bad_seal() {
    let result = fixtures::bad_seal_case().verify();
    assert_lock_script_exit(result, 50);
}

#[test]
fn contract_rejects_malformed_cobuild_witness() {
    let result = fixtures::malformed_cobuild_witness_case().verify();
    assert_lock_script_exit(result, 30);
}

#[test]
fn contract_rejects_malformed_otx_layout() {
    let result = fixtures::malformed_otx_layout_case().verify();
    assert_lock_script_exit(result, 31);
}

fn assert_lock_script_exit(result: Result<u64, ckb_testtool::ckb_error::Error>, code: i8) {
    use ckb_testtool::{
        ckb_error::ErrorKind,
        ckb_script::{ScriptError, TransactionScriptError},
    };

    let err = result.expect_err("transaction must fail closed");
    assert_eq!(err.kind(), ErrorKind::Script);

    let script_error = err
        .root_cause()
        .downcast_ref::<TransactionScriptError>()
        .expect("script validation error");
    assert_eq!(
        script_error.originating_script().to_string(),
        "Inputs[0].Lock"
    );
    assert!(
        matches!(
            script_error.script_error(),
            ScriptError::ValidationFailure(_, actual) if *actual == code
        ),
        "{script_error:?}"
    );
}
