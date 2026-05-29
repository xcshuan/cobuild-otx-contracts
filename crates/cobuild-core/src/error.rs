#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreError {
    MalformedCobuild,
    InvalidLayout,
    InvalidMessageTarget,
    MissingHashParts,
    MissingSealPair,
    DuplicateSealPair,
}
