use ckb_std::error::SysError;

#[repr(i8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    IndexOutOfBound = 1,
    ItemMissing = 2,
    LengthNotEnough = 3,
    Encoding = 4,
    InvalidArgs = 5,
    NftData = 6,
    UnexpectedSyscall = 7,
    TypeId = 8,
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
            SysError::Unknown(_) => Self::UnexpectedSyscall,
            SysError::WaitFailure
            | SysError::InvalidFd
            | SysError::OtherEndClosed
            | SysError::MaxVmsSpawned
            | SysError::MaxFdsCreated => Self::UnexpectedSyscall,
        }
    }
}

impl From<Error> for i8 {
    fn from(err: Error) -> Self {
        err as i8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_sys_error_maps_to_unexpected_syscall() {
        assert_eq!(
            Error::from(SysError::Unknown(255)),
            Error::UnexpectedSyscall
        );
    }
}
