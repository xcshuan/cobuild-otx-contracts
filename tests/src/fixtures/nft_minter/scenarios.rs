use ckb_hash::new_blake2b;
use ckb_testtool::ckb_types::{bytes::Bytes, packed::CellInput, prelude::*};

use crate::{
    fixtures::{
        cobuild_otx_lock::CobuildOtxLockError,
        common::contracts::{
            deploy_always_success, deploy_cobuild_otx_lock, deploy_minted_nft_type,
            deploy_nft_minter_type, rebuild_data2_script,
        },
        nft_minter::{
            MintedNftData, MintedNftTypeError, MinterState, NftMinterExpected, NftMinterTypeError,
            attributes_hash, create_minter_action_data, mint_nft_action_data, minted_nft_data,
            minter_data, nft_id, rarity_for_serial,
        },
    },
    framework::{
        cells::{TestCellOutput, live_resolved_facts, normal_output, typed_output},
        cobuild::{ActionRole, CobuildMessageBuilder},
        fixture::CobuildTestFixture,
        scripts::script_hash,
        signing::{
            SignatureScope, SignerId, TestSigningHashOracle, fixed_secret_key, public_key_hash20,
            sign_scope,
        },
        tx::{BuiltTxShape, OtxSegment, ProtocolMutation, TxShape, TxShapeMutation},
    },
};

mod create;
mod mint;
mod minted_nft;
mod minter_lifecycle;
mod otx;

pub use create::{
    create_minter_case, create_minter_missing_action_case, create_minter_non_zero_counter_case,
    create_minter_supply_cap_mismatch_case,
};
pub use mint::{
    mint_duplicate_nft_output_case, mint_first_nft_case, mint_from_counter_six_case,
    mint_missing_nft_output_case, mint_reaches_supply_cap_case, mint_serial_eleven_case,
    mint_serial_seven_case, mint_serial_seventy_seven_case, mint_supply_cap_changes_case,
    mint_supply_cap_overrun_case, mint_two_actions_tx_level_case, mint_wrong_attributes_case,
    mint_wrong_counter_case, mint_wrong_minter_hash_case, mint_wrong_rarity_case,
    mint_wrong_serial_case,
};
pub use minted_nft::{
    forged_nft_creation_case, nft_burn_case, nft_create_serial_outside_minter_transition_case,
    nft_create_wrong_args_case, nft_multiple_group_inputs_case, nft_multiple_group_outputs_case,
    nft_transfer_mutates_data_case, nft_valid_transfer_case,
};
pub use minter_lifecycle::{
    minter_burn_case, minter_multiple_group_inputs_case, minter_multiple_group_outputs_case,
};
pub use otx::{
    mint_mixed_tx_and_otx_order_case, mint_otx_output_in_other_otx_append_range_case,
    mint_otx_output_in_remainder_case, mint_otx_output_outside_append_range_case,
    mint_real_otx_lock_bad_base_seal_case, mint_real_otx_lock_missing_base_seal_case,
    mint_real_otx_lock_signed_base_case, mint_real_otx_lock_tampered_base_output_case,
};

pub struct NftMinterCase {
    pub name: &'static str,
    pub fixture: CobuildTestFixture,
    pub built: BuiltTxShape,
    pub expected: NftMinterExpected,
}

impl NftMinterCase {
    pub fn assert_expected(&self) {
        self.expected.assert(&self.fixture, &self.built);
    }

    pub fn assert_expected_with_context(&self) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.assert_expected();
        }));
        if let Err(payload) = result {
            std::panic::resume_unwind(Box::new(format!(
                "nft minter case `{}` failed: {}",
                self.name,
                panic_message(payload)
            )));
        }
    }
}

fn minted_nft_output(
    lock_script: &ckb_testtool::ckb_types::packed::Script,
    nft_script: &ckb_testtool::ckb_types::packed::Script,
    minter_hash: [u8; 32],
    serial: u64,
    seed: [u8; 32],
) -> TestCellOutput {
    let rarity = rarity_for_serial(serial);
    TestCellOutput::new(
        typed_output(lock_script.clone(), nft_script.clone(), 200_000_000_000),
        minted_nft_data(MintedNftData {
            minter_type_hash: minter_hash,
            serial,
            rarity,
            attributes_hash: attributes_hash(minter_hash, serial, rarity, seed),
        }),
    )
}

fn type_id_args(first_input: &CellInput, output_index: u64) -> [u8; 32] {
    let mut blake2b = new_blake2b();
    blake2b.update(first_input.as_slice());
    blake2b.update(&output_index.to_le_bytes());
    let mut out = [0u8; 32];
    blake2b.finalize(&mut out);
    out
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_owned()
    } else {
        "non-string panic payload".to_owned()
    }
}
