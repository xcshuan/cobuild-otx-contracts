use std::env;

use ckb_testtool::{
    ckb_error::{Error, ErrorKind},
    ckb_script::{ScriptError, TransactionScriptError},
    ckb_types::core::{Cycle, TransactionView},
    context::Context,
};

use crate::verify_and_dump_failed_tx;

const MAX_CYCLES: u64 = 50_000_000;
const DUMP_EXPECTED_FAILURES_ENV: &str = "COBUILD_TEST_DUMP_EXPECTED_FAILURES";

pub fn assert_pass(context: &Context, tx: &TransactionView) -> Cycle {
    let result = verify_and_dump_failed_tx(context, tx, MAX_CYCLES);
    assert!(result.is_ok(), "{result:?}");
    result.expect("transaction should pass")
}

pub fn assert_type_script_exit(
    context: &Context,
    tx: &TransactionView,
    input_index: usize,
    code: i8,
) {
    let result = context.verify_tx(tx, MAX_CYCLES);
    if result.is_err() && dump_expected_failures() {
        let _ = verify_and_dump_failed_tx(context, tx, MAX_CYCLES);
    }
    assert_type_script_exit_result(result, input_index, code);
}

pub fn assert_type_script_exit_result(result: Result<Cycle, Error>, input_index: usize, code: i8) {
    assert_script_exit_result(result, format!("Inputs[{input_index}].Type"), code);
}

pub fn assert_output_type_script_exit(
    context: &Context,
    tx: &TransactionView,
    output_index: usize,
    code: i8,
) {
    let result = context.verify_tx(tx, MAX_CYCLES);
    if result.is_err() && dump_expected_failures() {
        let _ = verify_and_dump_failed_tx(context, tx, MAX_CYCLES);
    }
    assert_output_type_script_exit_result(result, output_index, code);
}

pub fn assert_output_type_script_exit_result(
    result: Result<Cycle, Error>,
    output_index: usize,
    code: i8,
) {
    assert_script_exit_result(result, format!("Outputs[{output_index}].Type"), code);
}

pub fn assert_lock_script_exit(
    context: &Context,
    tx: &TransactionView,
    input_index: usize,
    code: i8,
) {
    let result = context.verify_tx(tx, MAX_CYCLES);
    if result.is_err() && dump_expected_failures() {
        let _ = verify_and_dump_failed_tx(context, tx, MAX_CYCLES);
    }
    assert_lock_script_exit_result(result, input_index, code);
}

pub fn assert_lock_script_exit_result(result: Result<Cycle, Error>, input_index: usize, code: i8) {
    assert_script_exit_result(result, format!("Inputs[{input_index}].Lock"), code);
}

fn assert_script_exit_result(result: Result<Cycle, Error>, originating_script: String, code: i8) {
    let err = result.expect_err("transaction must fail closed");
    assert_eq!(err.kind(), ErrorKind::Script);

    let script_error = err
        .root_cause()
        .downcast_ref::<TransactionScriptError>()
        .expect("script validation error");
    assert_eq!(
        script_error.originating_script().to_string(),
        originating_script,
        "originating script"
    );
    assert!(
        matches!(
            script_error.script_error(),
            ScriptError::ValidationFailure(_, actual) if *actual == code
        ),
        "exit code: {script_error:?}"
    );
}

fn dump_expected_failures() -> bool {
    env::var(DUMP_EXPECTED_FAILURES_ENV).as_deref() == Ok("1")
}
