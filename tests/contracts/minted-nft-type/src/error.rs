use ckb_std::error::SysError;

#[repr(i8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Syscall = 5,
    InvalidArgs = 10,
    InvalidNftData = 11,
    InvalidMinterTransition = 12,
    InvalidShape = 13,
}

impl From<SysError> for Error {
    fn from(_: SysError) -> Self {
        Self::Syscall
    }
}

impl From<Error> for i8 {
    fn from(error: Error) -> Self {
        error as i8
    }
}
