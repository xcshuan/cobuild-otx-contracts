#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CobuildOtxLockError {
    InvalidArgs,
    MalformedCobuildWitness,
    MalformedOtxLayout,
    InvalidMessageTarget,
    MissingLockGroupCoverage,
    MissingSealPair,
    DuplicateSealPair,
    InvalidSealScope,
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
            Self::MissingSealPair => 35,
            Self::DuplicateSealPair => 36,
            Self::InvalidSealScope => 37,
            Self::InvalidLockGroupWitness => 39,
            Self::NoRelevantSignatureRequest => 40,
            Self::BadSeal => 50,
        }
    }
}
