use tests::fixtures;

#[test]
fn contract_rejects_invalid_args() {
    let result = fixtures::invalid_args_case().verify();
    assert_lock_script_exit(result, 1);
}

#[test]
fn contract_rejects_without_relevant_task() {
    let result = fixtures::no_relevant_task_case().verify();
    assert_lock_script_exit(result, 3);
}

#[test]
fn contract_accepts_tx_level_cobuild_signature() {
    let result = fixtures::signed_tx_level_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_otx_base_and_append_signatures() {
    let result = fixtures::signed_otx_dual_scope_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_mixed_tx_level_and_otx_tasks() {
    let result = fixtures::mixed_tx_and_otx_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_rejects_bad_seal() {
    let result = fixtures::bad_seal_case().verify();
    assert_lock_script_exit(result, 4);
}

#[test]
fn contract_rejects_malformed_cobuild_witness() {
    let result = fixtures::malformed_cobuild_witness_case().verify();
    assert_lock_script_exit(result, 2);
}

#[test]
fn contract_rejects_malformed_otx_layout_as_malformed_cobuild() {
    let result = fixtures::malformed_otx_layout_case().verify();
    assert_lock_script_exit(result, 2);
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
