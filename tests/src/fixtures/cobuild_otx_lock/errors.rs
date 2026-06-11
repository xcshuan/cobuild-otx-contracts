#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CobuildOtxLockError {
    InvalidArgs,
    MalformedCobuildWitness,
    MalformedOtxLayout,
    NoRelevantSignatureRequest,
    BadSeal,
}

impl CobuildOtxLockError {
    pub fn code(self) -> i8 {
        match self {
            Self::InvalidArgs => 20,
            Self::MalformedCobuildWitness => 30,
            Self::MalformedOtxLayout => 31,
            Self::NoRelevantSignatureRequest => 40,
            Self::BadSeal => 50,
        }
    }
}
