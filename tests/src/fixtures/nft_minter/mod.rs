mod errors;
mod scenarios;
mod state;

pub use errors::{MintedNftTypeError, NftMinterExpected, NftMinterTypeError};
pub use scenarios::{
    NftMinterCase, create_minter_case, create_minter_missing_action_case,
    create_minter_non_zero_counter_case, create_minter_real_sighash_all_bad_seal_case,
    create_minter_real_sighash_all_signed_case,
    create_minter_real_sighash_all_tampered_output_case, create_minter_supply_cap_mismatch_case,
    forged_nft_creation_case, mint_duplicate_nft_output_case, mint_first_nft_case,
    mint_from_counter_six_case, mint_missing_nft_output_case, mint_mixed_tx_and_otx_order_case,
    mint_otx_output_in_other_otx_append_range_case, mint_otx_output_in_remainder_case,
    mint_otx_output_outside_append_range_case, mint_reaches_supply_cap_case,
    mint_real_otx_lock_bad_base_seal_case, mint_real_otx_lock_missing_base_seal_case,
    mint_real_otx_lock_signed_base_case,
    mint_real_otx_lock_tampered_append_nft_output_signed_base_case,
    mint_real_otx_lock_tampered_base_output_case, mint_serial_eleven_case, mint_serial_seven_case,
    mint_serial_seventy_seven_case, mint_supply_cap_changes_case, mint_supply_cap_overrun_case,
    mint_three_otx_actions_single_minter_transition_signed_base_case,
    mint_two_actions_tx_level_case, mint_wrong_attributes_case, mint_wrong_counter_case,
    mint_wrong_minter_hash_case, mint_wrong_rarity_case, mint_wrong_serial_case, minter_burn_case,
    minter_multiple_group_inputs_case, minter_multiple_group_outputs_case, nft_burn_case,
    nft_create_serial_outside_minter_transition_case, nft_create_wrong_args_case,
    nft_multiple_group_inputs_case, nft_multiple_group_outputs_case,
    nft_transfer_mutates_data_case, nft_valid_transfer_case,
};
pub use state::{
    CREATE_ACTION_LEN, CREATE_MINTER_TAG, MINT_ACTION_LEN, MINT_NFT_TAG, MINTER_DATA_LEN,
    MintedNftData, MinterState, NFT_DATA_LEN, attributes_hash, create_minter_action_data,
    mint_nft_action_data, minted_nft_data, minter_data, nft_id, rarity_for_serial,
};
