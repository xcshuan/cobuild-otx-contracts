mod cases;
mod errors;

pub use cases::{BuiltCobuildOtxLockCase, TwoUdtTransferFacts, cases, two_udt_transfer_otxs_case};
pub use errors::CobuildOtxLockError;

pub fn assert_coverage_manifest(cases: &[BuiltCobuildOtxLockCase]) {
    assert_eq!(cases.len(), 22, "cobuild otx lock case coverage count");
    assert!(
        cases.iter().any(|case| case.name
            == "contract_accepts_other_lock_outside_otx_without_tx_level_signature"),
        "cobuild otx lock coverage must include unrelated outside lock inputs"
    );
}
