use ckb_std::error::SysError;
use cobuild_core::error::CoreError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    SysIndexOutOfBound,
    SysItemMissing,
    SysLengthNotEnough,
    SysEncoding,
    SysWaitFailure,
    SysInvalidFd,
    SysOtherEndClosed,
    SysMaxVmsSpawned,
    SysMaxFdsCreated,
    SyscallUnknown,
    InvalidArgs,
    MalformedCobuild,
    InvalidOtxLayout,
    InvalidMessageTarget,
    DuplicateSighashAll,
    MissingLockGroupCoverage,
    MissingSealPair,
    DuplicateSealPair,
    InvalidSealScope,
    DuplicateMatchingAction,
    InvalidLockGroupWitness,
    LockSemanticFailure,
    ActionNotFound,
    VerifyFailure,
    InternalFailure,
    InvalidContextInput,
    MissingHashInput,
    HashInputTooLarge,
}

impl Error {
    pub const fn code(self) -> i8 {
        match self {
            Self::SysIndexOutOfBound => 1,
            Self::SysItemMissing => 2,
            Self::SysLengthNotEnough => 3,
            Self::SysEncoding => 4,
            Self::SysWaitFailure => 5,
            Self::SysInvalidFd => 6,
            Self::SysOtherEndClosed => 7,
            Self::SysMaxVmsSpawned => 8,
            Self::SysMaxFdsCreated => 9,
            Self::SyscallUnknown => 10,

            Self::InvalidArgs => 20,

            Self::MalformedCobuild => 30,
            Self::InvalidOtxLayout => 31,
            Self::InvalidMessageTarget => 32,
            Self::DuplicateSighashAll => 33,
            Self::MissingLockGroupCoverage => 34,
            Self::MissingSealPair => 35,
            Self::DuplicateSealPair => 36,
            Self::InvalidSealScope => 37,
            Self::DuplicateMatchingAction => 38,
            Self::InvalidLockGroupWitness => 39,

            Self::LockSemanticFailure => 40,
            Self::ActionNotFound => 41,

            Self::VerifyFailure => 50,

            Self::InternalFailure => 60,
            Self::InvalidContextInput => 61,
            Self::MissingHashInput => 62,
            Self::HashInputTooLarge => 63,
        }
    }
}

impl From<Error> for i8 {
    fn from(error: Error) -> Self {
        error.code()
    }
}

impl From<SysError> for Error {
    fn from(error: SysError) -> Self {
        match error {
            SysError::IndexOutOfBound => Self::SysIndexOutOfBound,
            SysError::ItemMissing => Self::SysItemMissing,
            SysError::LengthNotEnough(_) => Self::SysLengthNotEnough,
            SysError::Encoding => Self::SysEncoding,
            SysError::WaitFailure => Self::SysWaitFailure,
            SysError::InvalidFd => Self::SysInvalidFd,
            SysError::OtherEndClosed => Self::SysOtherEndClosed,
            SysError::MaxVmsSpawned => Self::SysMaxVmsSpawned,
            SysError::MaxFdsCreated => Self::SysMaxFdsCreated,
            #[cfg(feature = "type-id")]
            SysError::TypeIDError => Self::SyscallUnknown,
            SysError::Unknown(_) => Self::SyscallUnknown,
        }
    }
}

impl From<CoreError> for Error {
    fn from(error: CoreError) -> Self {
        match error {
            CoreError::MalformedCobuild => Self::MalformedCobuild,
            CoreError::InvalidOtxLayout => Self::InvalidOtxLayout,
            CoreError::InvalidContextInput => Self::InvalidContextInput,
            CoreError::InvalidMessageTarget => Self::InvalidMessageTarget,
            CoreError::MissingHashInput => Self::MissingHashInput,
            CoreError::HashInputTooLarge => Self::HashInputTooLarge,
            CoreError::DuplicateSighashAll => Self::DuplicateSighashAll,
            CoreError::MissingLockGroupCoverage => Self::MissingLockGroupCoverage,
            CoreError::MissingSealPair => Self::MissingSealPair,
            CoreError::DuplicateSealPair => Self::DuplicateSealPair,
            CoreError::InvalidSealScope => Self::InvalidSealScope,
            CoreError::DuplicateMatchingAction => Self::DuplicateMatchingAction,
            CoreError::InvalidLockGroupWitness => Self::InvalidLockGroupWitness,
            CoreError::ActionNotFound => Self::ActionNotFound,
        }
    }
}
