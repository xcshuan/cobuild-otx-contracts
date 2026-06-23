mod cases;
mod errors;

pub use cases::{BuiltCobuildOtxLockCase, TwoUdtTransferFacts, cases, two_udt_transfer_otxs_case};
pub use errors::CobuildOtxLockError;

pub fn assert_coverage_manifest(cases: &[BuiltCobuildOtxLockCase]) {
    assert_eq!(cases.len(), 30, "cobuild otx lock case coverage count");
    assert!(
        cases.iter().any(|case| case.name
            == "contract_accepts_other_lock_outside_otx_without_tx_level_signature"),
        "cobuild otx lock coverage must include unrelated outside lock inputs"
    );
    assert!(
        cases
            .iter()
            .any(|case| case.name == "contract_accepts_nft_for_udt_swap_otxs_in_one_transaction"),
        "cobuild otx lock coverage must include composed NFT-for-UDT swap OTXs"
    );
    assert!(
        cases.iter().any(|case| {
            case.name == "contract_rejects_partial_output_mask_when_covered_lock_changes"
        }),
        "cobuild otx lock coverage must include partial mask real signature checks"
    );
}
