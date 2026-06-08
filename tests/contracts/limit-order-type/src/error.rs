#![allow(unexpected_cfgs)]

use ckb_std::error::SysError;
use cobuild_core::error::CoreError;

#[repr(i8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    IndexOutOfBound = 1,
    ItemMissing = 2,
    LengthNotEnough = 3,
    Encoding = 4,
    InvalidOrderData = 5,
    InvalidSettlementData = 6,
    InvalidActionData = 7,
    UnsupportedAction = 8,
    AmountOverflow = 9,
    ActionMismatch = 10,
    InsufficientPayment = 11,
    InvalidCobuild = 12,
    UnexpectedSyscall = 13,
    TypeId = 14,
}

impl From<SysError> for Error {
    fn from(err: SysError) -> Self {
        match err {
            SysError::IndexOutOfBound => Self::IndexOutOfBound,
            SysError::ItemMissing => Self::ItemMissing,
            SysError::LengthNotEnough(_) => Self::LengthNotEnough,
            SysError::Encoding => Self::Encoding,
            #[cfg(feature = "type-id")]
            SysError::TypeIDError => Self::TypeId,
            SysError::Unknown(code) => panic!("unknown syscall error {code}"),
            SysError::WaitFailure
            | SysError::InvalidFd
            | SysError::OtherEndClosed
            | SysError::MaxVmsSpawned
            | SysError::MaxFdsCreated => Self::UnexpectedSyscall,
            #[allow(unreachable_patterns)]
            _ => Self::UnexpectedSyscall,
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
