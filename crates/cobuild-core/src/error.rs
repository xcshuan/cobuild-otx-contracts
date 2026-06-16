#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreError {
    MalformedCobuild,
    InvalidOtxLayout,
    InvalidContextInput,
    InvalidMessageTarget,
    MissingHashInput,
    HashInputTooLarge,
    ActionNotFound,
    DuplicateSighashAll,
    MissingLockGroupCoverage,
    MissingSealPair,
    DuplicateSealPair,
    InvalidSealScope,
    DuplicateMatchingAction,
    InvalidLockGroupWitness,
}
