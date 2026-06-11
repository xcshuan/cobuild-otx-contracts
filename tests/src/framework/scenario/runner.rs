use crate::framework::{fixture::CobuildTestFixture, tx::BuiltTxShape};

use super::ExpectedOutcome;

pub fn assert_expected_outcome(
    fixture: &CobuildTestFixture,
    built: &BuiltTxShape,
    expected: &ExpectedOutcome,
) {
    expected.assert(fixture, built);
}
