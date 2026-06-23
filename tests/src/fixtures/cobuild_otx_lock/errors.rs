#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CobuildOtxLockError {
    InvalidArgs,
    MalformedCobuildWitness,
    MalformedOtxLayout,
    InvalidMessageTarget,
    MissingLockGroupCoverage,
    MissingLockSeal,
    DuplicateLockSeal,
    InvalidLockGroupWitness,
    NoRelevantSignatureRequest,
    BadSeal,
}

impl CobuildOtxLockError {
    pub fn code(self) -> i8 {
        match self {
            Self::InvalidArgs => 20,
            Self::MalformedCobuildWitness => 30,
            Self::MalformedOtxLayout => 31,
            Self::InvalidMessageTarget => 32,
            Self::MissingLockGroupCoverage => 34,
            Self::MissingLockSeal => 35,
            Self::DuplicateLockSeal => 36,
            Self::InvalidLockGroupWitness => 39,
            Self::NoRelevantSignatureRequest => 40,
            Self::BadSeal => 50,
        }
    }
}
