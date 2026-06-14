use ckb_std::error::SysError;

#[repr(i8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Syscall = 5,
    InvalidArgs = 10,
    TypeId = 11,
    InvalidMinterData = 12,
    InvalidAction = 13,
    InvalidCobuild = 14,
    InvalidMintedNft = 15,
    Counter = 16,
    SupplyCap = 17,
    InvalidShape = 18,
}

impl From<SysError> for Error {
    fn from(error: SysError) -> Self {
        match error {
            #[cfg(feature = "type-id")]
            SysError::TypeIDError => Self::TypeId,
            _ => Self::Syscall,
        }
    }
}

impl From<cobuild_core::error::CoreError> for Error {
    fn from(_: cobuild_core::error::CoreError) -> Self {
        Self::InvalidCobuild
    }
}

impl From<Error> for i8 {
    fn from(error: Error) -> Self {
        error as i8
    }
}
