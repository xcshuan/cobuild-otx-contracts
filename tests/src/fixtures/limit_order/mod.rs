mod actions;
mod errors;
mod lock_nft_for_udt;
mod mutations;
mod scenarios;
mod state;
mod type_nft_for_udt;

pub use actions::{LimitOrderAction, create_order_action_data, encode_action};
pub use errors::{LimitOrderExpectedOutcome, LimitOrderLockError, LimitOrderTypeError};
pub use lock_nft_for_udt::{lock_script_cases, lock_script_fill_cases, mixed_type_lock_cases};
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

use crate::framework::{contracts::DeployedScript, fixture::CobuildTestFixture};

use super::common::contracts::deploy_limit_order_type;

pub use crate::framework::assertions::failed_txs_count;

const OFFERED_ASSET_ID: [u8; 32] = [3; 32];
const REQUESTED_ASSET_ID: [u8; 32] = [4; 32];
pub(crate) const NFT_TYPE_ARGS: [u8; 32] = [5; 32];

pub trait LimitOrderFixtureExt {
    fn deploy_limit_order(&mut self) -> DeployedScript;
}

impl LimitOrderFixtureExt for CobuildTestFixture {
    fn deploy_limit_order(&mut self) -> DeployedScript {
        deploy_limit_order_type(self.context_mut())
    }
}
