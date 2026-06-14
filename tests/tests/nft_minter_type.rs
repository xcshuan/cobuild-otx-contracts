use tests::fixtures::nft_minter::{
    create_minter_case, create_minter_missing_action_case, create_minter_non_zero_counter_case,
    create_minter_supply_cap_mismatch_case, forged_nft_creation_case, mint_first_nft_case,
    mint_from_counter_six_case, mint_missing_nft_output_case, mint_wrong_attributes_case,
    mint_wrong_counter_case, minter_burn_case, nft_transfer_mutates_data_case,
};

#[test]
fn nft_minter_cases_match_expected_outcomes() {
    for case in [
        create_minter_case(),
        mint_first_nft_case(),
        mint_from_counter_six_case(),
        create_minter_missing_action_case(),
        create_minter_non_zero_counter_case(),
        create_minter_supply_cap_mismatch_case(),
        mint_wrong_counter_case(),
        mint_missing_nft_output_case(),
        mint_wrong_attributes_case(),
        forged_nft_creation_case(),
        nft_transfer_mutates_data_case(),
        minter_burn_case(),
    ] {
        case.assert_expected_with_context();
    }
}
