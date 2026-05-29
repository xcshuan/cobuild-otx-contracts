#[repr(i8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitCode {
    InvalidArgs = 1,
    MalformedCobuild = 2,
    LockSemanticFailure = 3,
    VerifyFailure = 4,
    SyscallFailure = 5,
    InternalFailure = 6,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidArgs,
    MalformedCobuild,
    LockSemanticFailure,
    VerifyFailure,
    SyscallFailure,
    InternalFailure,
}

impl Error {
    pub fn exit_code(&self) -> i8 {
        match self {
            Self::InvalidArgs => ExitCode::InvalidArgs as i8,
            Self::MalformedCobuild => ExitCode::MalformedCobuild as i8,
            Self::LockSemanticFailure => ExitCode::LockSemanticFailure as i8,
            Self::VerifyFailure => ExitCode::VerifyFailure as i8,
            Self::SyscallFailure => ExitCode::SyscallFailure as i8,
            Self::InternalFailure => ExitCode::InternalFailure as i8,
        }
    }
}
