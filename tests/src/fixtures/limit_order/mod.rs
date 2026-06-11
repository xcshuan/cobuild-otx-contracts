use ckb_testtool::{
    ckb_types::packed::{CellInput, Script},
    context::Context,
};

mod actions;
mod errors;
mod lock_nft_for_udt;
mod mutations;
mod scenarios;
mod state;
mod type_nft_for_udt;

pub use actions::{LimitOrderAction, create_order_action_data, encode_action};
pub use errors::{LimitOrderExpectedOutcome, LimitOrderLockError, LimitOrderTypeError};
pub use lock_nft_for_udt::{
    LimitOrderLockFillCase, limit_order_lock_nft_for_udt_case,
    limit_order_lock_nft_for_udt_case_with, limit_order_lock_otx_with_sighash_all_fill_case,
    mixed_limit_order_type_lock_duplicate_payment_case,
};
pub use mutations::BusinessMutation;
pub use scenarios::{
    ActionSourceKind, BuiltLimitOrderCase, CoverageTag, FlowKind, LimitOrderHappyPath,
    OtxScopeKind, ScriptRoleKind,
};
pub use state::{LimitOrderState, order_data, settlement_data};
pub use type_nft_for_udt::{
    type_script_cases, type_script_create_order_cases, type_script_fill_cases,
    type_script_legacy_settlement_cases,
};

use crate::framework::{
    cells::{live_input, typed_output},
    contracts::DeployedScript,
    fixture::CobuildTestFixture,
    scripts::script_hash,
};

use super::common::contracts::deploy_limit_order_type;

pub use crate::framework::assertions::failed_txs_count;

const OFFERED_ASSET_ID: [u8; 32] = [3; 32];
const REQUESTED_ASSET_ID: [u8; 32] = [4; 32];
pub(crate) const NFT_TYPE_ARGS: [u8; 32] = [5; 32];

pub trait LimitOrderFixtureExt {
    fn deploy_limit_order(&mut self) -> DeployedScript;
    fn limit_order(&mut self) -> LimitOrderBuilder<'_>;
}

impl LimitOrderFixtureExt for CobuildTestFixture {
    fn deploy_limit_order(&mut self) -> DeployedScript {
        deploy_limit_order_type(self.context_mut())
    }

    fn limit_order(&mut self) -> LimitOrderBuilder<'_> {
        LimitOrderBuilder::new(self.context_mut())
    }
}

pub struct LimitOrderBuilder<'a> {
    context: &'a mut Context,
    owner: Option<Script>,
    offered_nft_type_hash: [u8; 32],
    requested_asset_id: [u8; 32],
    requested_amount: u64,
    capacity: u64,
}

impl<'a> LimitOrderBuilder<'a> {
    fn new(context: &'a mut Context) -> Self {
        Self {
            context,
            owner: None,
            offered_nft_type_hash: [3; 32],
            requested_asset_id: [4; 32],
            requested_amount: 30,
            capacity: 100_000_000_000,
        }
    }

    pub fn owner(mut self, owner: Script) -> Self {
        self.owner = Some(owner);
        self
    }

    pub fn offered_nft_type_hash(mut self, type_hash: [u8; 32]) -> Self {
        self.offered_nft_type_hash = type_hash;
        self
    }

    pub fn requested_asset_id(mut self, asset_id: [u8; 32]) -> Self {
        self.requested_asset_id = asset_id;
        self
    }

    pub fn requested_amount(mut self, amount: u64) -> Self {
        self.requested_amount = amount;
        self
    }

    pub fn build_input(self, limit_order_type: &Script) -> CellInput {
        let owner = self.owner.expect("limit order owner lock");
        let owner_lock_hash = script_hash(&owner);
        let output = typed_output(owner, limit_order_type.clone(), self.capacity);
        let data = order_data(LimitOrderState {
            owner_lock_hash,
            offered_nft_type_hash: self.offered_nft_type_hash,
            requested_asset_id: self.requested_asset_id,
            requested_amount: self.requested_amount,
        });
        live_input(self.context, output, data)
    }
}
