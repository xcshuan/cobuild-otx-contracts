#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreError {
    MalformedCobuild,
    InvalidOtxLayout,
    InvalidContextInput,
    InvalidMessageTarget,
    MissingHashInput,
    HashInputTooLarge,
    DuplicateSighashAll,
    MissingLockGroupCoverage,
    MissingSealPair,
    DuplicateSealPair,
    InvalidSealScope,
}
