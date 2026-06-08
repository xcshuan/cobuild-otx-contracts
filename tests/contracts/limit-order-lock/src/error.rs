use ckb_std::error::SysError;
use cobuild_core::error::CoreError;

#[repr(i8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    IndexOutOfBound = 1,
    ItemMissing = 2,
    LengthNotEnough = 3,
    Encoding = 4,
    InvalidArgs = 5,
    InvalidActionData = 6,
    UnsupportedAction = 7,
    InvalidNftInput = 8,
    ActionMismatch = 9,
    InsufficientPayment = 10,
    AmountOverflow = 11,
    InvalidCobuild = 12,
    UnexpectedSyscall = 13,
}

impl From<SysError> for Error {
    fn from(err: SysError) -> Self {
        match err {
            SysError::IndexOutOfBound => Self::IndexOutOfBound,
            SysError::ItemMissing => Self::ItemMissing,
            SysError::LengthNotEnough(_) => Self::LengthNotEnough,
            SysError::Encoding => Self::Encoding,
            SysError::Unknown(_) => Self::UnexpectedSyscall,
            SysError::WaitFailure
            | SysError::InvalidFd
            | SysError::OtherEndClosed
            | SysError::MaxVmsSpawned
            | SysError::MaxFdsCreated => Self::UnexpectedSyscall,
        }
    }
}

impl From<CoreError> for Error {
    fn from(_: CoreError) -> Self {
        Self::InvalidCobuild
    }
}

impl From<Error> for i8 {
    fn from(err: Error) -> Self {
        err as i8
    }
}
