use ckb_testtool::ckb_types::{
    bytes::Bytes,
    packed::{CellOutput, Script},
    prelude::*,
};
use cobuild_types::entity::{
    core::{Otx, SealPairVec},
    witness::{WitnessLayout, WitnessLayoutUnion},
};
use secp256k1::SecretKey;

use crate::{
    fixtures::{
        cobuild_otx_lock::CobuildOtxLockError,
        common::{
            assets::{nft_data, udt_amount_data},
            contracts::{
                build_cobuild_otx_lock, deploy_always_success, deploy_cobuild_otx_lock_code,
                deploy_test_nft, deploy_test_udt, rebuild_data2_script,
            },
        },
    },
    framework::{
        cells::{ResolvedInputFacts, TestCellOutput, live_resolved_facts, normal_output},
        cobuild::{
            BaseInputMaskField, BaseOutputMaskField, base_cell_dep_item_mask,
            base_header_dep_item_mask, base_input_mask, base_output_mask, full_base_cell_dep_masks,
            full_base_header_dep_masks, full_base_input_masks, full_base_output_masks, seal_pair,
        },
        fixture::CobuildTestFixture,
        scenario::{ExpectedOutcome, ScriptLocation},
        signing::{
            SignatureScope, SignerId, SigningFacts, TestSigningHashOracle, fixed_secret_key,
            public_key_hash20, sighash_all_only_witness, sign_scope,
        },
        tx::{
            BuiltTxShape, InputHandle, OtxHandle, OtxSegment, ProtocolMutation, TxShape,
            TxShapeMutation, WitnessHandle,
        },
    },
};

pub struct BuiltCobuildOtxLockCase {
    pub name: &'static str,
    pub fixture: CobuildTestFixture,
    pub built: BuiltTxShape,
    pub signing_facts: Vec<SigningFacts>,
    pub expected: ExpectedOutcome,
    pub two_udt_transfer_facts: Option<TwoUdtTransferFacts>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TwoUdtTransferFacts {
    pub fee_lock_hash: Option<[u8; 32]>,
    pub otx_a_lock_hash: [u8; 32],
    pub otx_b_lock_hash: [u8; 32],
}

mod basic;
mod helpers;
mod multi_otx;
mod otx_signatures;
mod partial_masks;
mod sighash_all;

use helpers::*;
pub use multi_otx::two_udt_transfer_otxs_case;

pub fn cases() -> Vec<BuiltCobuildOtxLockCase> {
    vec![
        basic::invalid_args_case(),
        basic::no_relevant_signature_request_case(),
        sighash_all::signed_sighash_all_case(),
        sighash_all::signed_sighash_all_offset_lock_case(),
        otx_signatures::signed_otx_dual_scope_case(),
        otx_signatures::signed_otx_full_preimage_case(),
        otx_signatures::signed_otx_append_output_mutated_after_signing_case(),
        otx_signatures::otx_and_outside_same_lock_without_tx_level_signature_case(),
        otx_signatures::otx_and_outside_other_lock_without_tx_level_signature_case(),
        otx_signatures::signed_otx_missing_base_seal_case(),
        otx_signatures::signed_otx_missing_append_seal_case(),
        otx_signatures::signed_otx_duplicate_base_seal_case(),
        otx_signatures::signed_otx_invalid_seal_scope_case(),
        otx_signatures::signed_otx_wrong_script_hash_seal_case(),
        otx_signatures::signed_otx_invalid_action_target_case(),
        otx_signatures::malformed_otx_duplicate_start_case(),
        partial_masks::input_mask_accepts_uncovered_previous_output_mutation_case(),
        partial_masks::input_mask_rejects_covered_previous_output_mutation_case(),
        partial_masks::output_mask_accepts_uncovered_lock_mutation_case(),
        partial_masks::output_mask_rejects_covered_lock_mutation_case(),
        partial_masks::cell_dep_mask_accepts_uncovered_cell_dep_mutation_case(),
        partial_masks::cell_dep_mask_rejects_covered_cell_dep_mutation_case(),
        partial_masks::header_dep_mask_accepts_uncovered_header_dep_mutation_case(),
        partial_masks::header_dep_mask_rejects_covered_header_dep_mutation_case(),
        multi_otx::two_udt_transfer_otxs_case(false),
        multi_otx::two_udt_transfer_otxs_case(true),
        multi_otx::nft_for_udt_swap_otxs_case(),
        otx_signatures::mixed_sighash_all_and_otx_case(),
        otx_signatures::bad_seal_case(),
        basic::malformed_cobuild_witness_case(),
        otx_signatures::malformed_otx_layout_case(),
    ]
}
