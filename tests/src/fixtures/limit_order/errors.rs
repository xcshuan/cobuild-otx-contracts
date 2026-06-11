use crate::framework::{
    fixture::CobuildTestFixture,
    scenario::{ExpectedOutcome, ScriptLocation},
    tx::{BuiltTxShape, InputHandle, OutputHandle},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitOrderTypeError {
    InputAndOutputGroupShape,
    StateActionMismatch,
    InvalidPayment,
    InvalidAction,
    InvalidTypeId,
}

impl LimitOrderTypeError {
    pub fn code(self) -> i8 {
        match self {
            Self::InputAndOutputGroupShape => 5,
            Self::StateActionMismatch => 10,
            Self::InvalidPayment => 11,
            Self::InvalidAction => 12,
            Self::InvalidTypeId => 14,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitOrderLockError {
    MalformedArgs,
    MalformedAction,
    UnknownActionTag,
    WrongNftType,
    InvalidPayment,
    InvalidAction,
}

impl LimitOrderLockError {
    pub fn code(self) -> i8 {
        match self {
            Self::MalformedArgs => 5,
            Self::MalformedAction => 6,
            Self::UnknownActionTag => 7,
            Self::WrongNftType => 8,
            Self::InvalidPayment => 10,
            Self::InvalidAction => 12,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LimitOrderExpectedOutcome {
    Pass,
    InputType {
        input: InputHandle,
        error: LimitOrderTypeError,
    },
    OutputType {
        output: OutputHandle,
        error: LimitOrderTypeError,
    },
    InputLock {
        input: InputHandle,
        error: LimitOrderLockError,
    },
    Framework(ExpectedOutcome),
}

impl LimitOrderExpectedOutcome {
    pub fn assert(&self, fixture: &CobuildTestFixture, built: &BuiltTxShape) {
        self.expected_outcome().assert(fixture, built);
    }

    pub fn expected_outcome(&self) -> ExpectedOutcome {
        match self {
            Self::Pass => ExpectedOutcome::Pass,
            Self::InputType { input, error } => ExpectedOutcome::ScriptExit {
                location: ScriptLocation::InputType(*input),
                code: error.code(),
            },
            Self::OutputType { output, error } => ExpectedOutcome::ScriptExit {
                location: ScriptLocation::OutputType(*output),
                code: error.code(),
            },
            Self::InputLock { input, error } => ExpectedOutcome::ScriptExit {
                location: ScriptLocation::InputLock(*input),
                code: error.code(),
            },
            Self::Framework(expected) => expected.clone(),
        }
    }
}
