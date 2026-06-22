use ckb_testtool::{
    ckb_error::{Error, ErrorKind},
    ckb_script::{ScriptError, TransactionScriptError},
    ckb_types::core::{Cycle, TransactionView},
};

use crate::framework::{
    assertions::{
        MAX_CYCLES, assert_lock_script_exit_result, assert_output_type_script_exit_result,
        assert_type_script_exit_result,
    },
    fixture::CobuildTestFixture,
    tx::{BuiltTxShape, InputHandle, OutputHandle},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptLocation {
    InputLock(InputHandle),
    InputType(InputHandle),
    OutputType(OutputHandle),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpectedOutcome {
    Pass,
    ScriptExit { location: ScriptLocation, code: i8 },
    AnyOf(Vec<ExpectedOutcome>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_outcome_can_express_alternative_script_exits() {
        let outcome = ExpectedOutcome::AnyOf(vec![
            ExpectedOutcome::ScriptExit {
                location: ScriptLocation::InputType(InputHandle(0)),
                code: 10,
            },
            ExpectedOutcome::ScriptExit {
                location: ScriptLocation::OutputType(OutputHandle(0)),
                code: 8,
            },
        ]);

        assert!(matches!(outcome, ExpectedOutcome::AnyOf(_)));
    }
}

impl ExpectedOutcome {
    pub fn assert(&self, fixture: &CobuildTestFixture, built: &BuiltTxShape) {
        match self {
            Self::Pass => fixture.assert_pass(&built.tx),
            Self::ScriptExit { location, code } => {
                self.assert_script_exit(fixture, &built.tx, built, *location, *code);
            }
            Self::AnyOf(candidates) => {
                let result = fixture.context().verify_tx(&built.tx, MAX_CYCLES);
                assert!(
                    candidates
                        .iter()
                        .any(|candidate| candidate.matches_result(&result, built)),
                    "result matched none of the expected outcomes: {result:?}"
                );
            }
        }
    }

    pub fn assert_result(&self, result: Result<Cycle, Error>, built: &BuiltTxShape) {
        match self {
            Self::Pass => {
                assert!(result.is_ok(), "{result:?}");
            }
            Self::ScriptExit { location, code } => match location {
                ScriptLocation::InputLock(input) => {
                    assert_lock_script_exit_result(result, built.inputs.tx_index(*input), *code);
                }
                ScriptLocation::InputType(input) => {
                    assert_type_script_exit_result(result, built.inputs.tx_index(*input), *code);
                }
                ScriptLocation::OutputType(output) => {
                    assert_output_type_script_exit_result(
                        result,
                        built.outputs.tx_index(*output),
                        *code,
                    );
                }
            },
            Self::AnyOf(candidates) => {
                assert!(
                    candidates
                        .iter()
                        .any(|candidate| candidate.matches_result(&result, built)),
                    "result matched none of the expected outcomes: {result:?}"
                );
            }
        }
    }

    fn matches_result(&self, result: &Result<Cycle, Error>, built: &BuiltTxShape) -> bool {
        match self {
            Self::Pass => result.is_ok(),
            Self::ScriptExit { location, code } => {
                script_exit_matches(result, expected_origin(*location, built), *code)
            }
            Self::AnyOf(candidates) => candidates
                .iter()
                .any(|candidate| candidate.matches_result(result, built)),
        }
    }

    fn assert_script_exit(
        &self,
        fixture: &CobuildTestFixture,
        tx: &TransactionView,
        built: &BuiltTxShape,
        location: ScriptLocation,
        code: i8,
    ) {
        match location {
            ScriptLocation::InputLock(input) => {
                fixture.assert_lock_script_exit(tx, built.inputs.tx_index(input), code);
            }
            ScriptLocation::InputType(input) => {
                fixture.assert_type_script_exit(tx, built.inputs.tx_index(input), code);
            }
            ScriptLocation::OutputType(output) => {
                fixture.assert_output_type_script_exit(tx, built.outputs.tx_index(output), code);
            }
        }
    }
}

fn expected_origin(location: ScriptLocation, built: &BuiltTxShape) -> String {
    match location {
        ScriptLocation::InputLock(input) => {
            format!("Inputs[{}].Lock", built.inputs.tx_index(input))
        }
        ScriptLocation::InputType(input) => {
            format!("Inputs[{}].Type", built.inputs.tx_index(input))
        }
        ScriptLocation::OutputType(output) => {
            format!("Outputs[{}].Type", built.outputs.tx_index(output))
        }
    }
}

fn script_exit_matches(
    result: &Result<Cycle, Error>,
    originating_script: String,
    code: i8,
) -> bool {
    let Err(err) = result else {
        return false;
    };
    if err.kind() != ErrorKind::Script {
        return false;
    }
    let Some(script_error) = err.root_cause().downcast_ref::<TransactionScriptError>() else {
        return false;
    };
    script_error.originating_script().to_string() == originating_script
        && matches!(
            script_error.script_error(),
            ScriptError::ValidationFailure(_, actual) if *actual == code
        )
}
