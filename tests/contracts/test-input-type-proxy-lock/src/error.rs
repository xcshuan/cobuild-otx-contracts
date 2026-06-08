use ckb_std::error::SysError;

#[repr(i8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    IndexOutOfBound = 1,
    ItemMissing = 2,
    LengthNotEnough = 3,
    Encoding = 4,
    InvalidUnlock = 5,
    UnexpectedSyscall = 6,
}

impl From<SysError> for Error {
    fn from(err: SysError) -> Self {
        match err {
            SysError::IndexOutOfBound => Self::IndexOutOfBound,
            SysError::ItemMissing => Self::ItemMissing,
            SysError::LengthNotEnough(_) => Self::LengthNotEnough,
            SysError::Encoding => Self::Encoding,
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
