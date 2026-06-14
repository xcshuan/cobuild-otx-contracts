use tests::fixtures::nft_minter::{
    create_minter_case, mint_first_nft_case, mint_from_counter_six_case,
};

#[test]
fn nft_minter_happy_paths_pass() {
    for case in [
        create_minter_case(),
        mint_first_nft_case(),
        mint_from_counter_six_case(),
    ] {
        case.assert_expected();
    }
}
