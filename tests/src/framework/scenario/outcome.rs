use ckb_testtool::{
    ckb_error::Error,
    ckb_types::core::{Cycle, TransactionView},
};

use crate::framework::{
    assertions::{
        assert_lock_script_exit_result, assert_output_type_script_exit_result,
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
}

impl ExpectedOutcome {
    pub fn assert(&self, fixture: &CobuildTestFixture, built: &BuiltTxShape) {
        match *self {
            Self::Pass => fixture.assert_pass(&built.tx),
            Self::ScriptExit { location, code } => {
                self.assert_script_exit(fixture, &built.tx, built, location, code);
            }
        }
    }

    pub fn assert_result(&self, result: Result<Cycle, Error>, built: &BuiltTxShape) {
        match *self {
            Self::Pass => {
                assert!(result.is_ok(), "{result:?}");
            }
            Self::ScriptExit { location, code } => match location {
                ScriptLocation::InputLock(input) => {
                    assert_lock_script_exit_result(result, built.inputs.tx_index(input), code);
                }
                ScriptLocation::InputType(input) => {
                    assert_type_script_exit_result(result, built.inputs.tx_index(input), code);
                }
                ScriptLocation::OutputType(output) => {
                    assert_output_type_script_exit_result(
                        result,
                        built.outputs.tx_index(output),
                        code,
                    );
                }
            },
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
