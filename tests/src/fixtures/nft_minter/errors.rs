use crate::framework::{
    fixture::CobuildTestFixture,
    scenario::{ExpectedOutcome, ScriptLocation},
    tx::{BuiltTxShape, InputHandle, OutputHandle},
};

use super::super::cobuild_otx_lock::CobuildOtxLockError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NftMinterTypeError {
    Syscall,
    InvalidArgs,
    TypeId,
    InvalidMinterData,
    InvalidAction,
    InvalidCobuild,
    InvalidMintedNft,
    Counter,
    SupplyCap,
    InvalidShape,
}

impl NftMinterTypeError {
    pub fn code(self) -> i8 {
        match self {
            Self::Syscall => 5,
            Self::InvalidArgs => 10,
            Self::TypeId => 11,
            Self::InvalidMinterData => 12,
            Self::InvalidAction => 13,
            Self::InvalidCobuild => 14,
            Self::InvalidMintedNft => 15,
            Self::Counter => 16,
            Self::SupplyCap => 17,
            Self::InvalidShape => 18,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MintedNftTypeError {
    Syscall,
    InvalidArgs,
    InvalidNftData,
    InvalidMinterTransition,
    InvalidShape,
}

impl MintedNftTypeError {
    pub fn code(self) -> i8 {
        match self {
            Self::Syscall => 5,
            Self::InvalidArgs => 10,
            Self::InvalidNftData => 11,
            Self::InvalidMinterTransition => 12,
            Self::InvalidShape => 13,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NftMinterExpected {
    Pass,
    MinterInputType {
        input: InputHandle,
        error: NftMinterTypeError,
    },
    MinterOutputType {
        output: OutputHandle,
        error: NftMinterTypeError,
    },
    MintedNftInputType {
        input: InputHandle,
        error: MintedNftTypeError,
    },
    MintedNftOutputType {
        output: OutputHandle,
        error: MintedNftTypeError,
    },
    OtxLockInput {
        input: InputHandle,
        error: CobuildOtxLockError,
    },
}

impl NftMinterExpected {
    pub fn assert(&self, fixture: &CobuildTestFixture, built: &BuiltTxShape) {
        self.expected_outcome().assert(fixture, built);
    }

    pub fn expected_outcome(&self) -> ExpectedOutcome {
        match self {
            Self::Pass => ExpectedOutcome::Pass,
            Self::MinterInputType { input, error } => ExpectedOutcome::ScriptExit {
                location: ScriptLocation::InputType(*input),
                code: error.code(),
            },
            Self::MinterOutputType { output, error } => ExpectedOutcome::ScriptExit {
                location: ScriptLocation::OutputType(*output),
                code: error.code(),
            },
            Self::MintedNftInputType { input, error } => ExpectedOutcome::ScriptExit {
                location: ScriptLocation::InputType(*input),
                code: error.code(),
            },
            Self::MintedNftOutputType { output, error } => ExpectedOutcome::ScriptExit {
                location: ScriptLocation::OutputType(*output),
                code: error.code(),
            },
            Self::OtxLockInput { input, error } => ExpectedOutcome::ScriptExit {
                location: ScriptLocation::InputLock(*input),
                code: error.code(),
            },
        }
    }
}
