use ckb_testtool::{
    ckb_types::{
        core::TransactionView,
        packed::{CellInput, Script},
    },
    context::Context,
};

#[cfg(not(test))]
mod nft_for_udt;

#[cfg(not(test))]
pub use nft_for_udt::{
    FillActionCase, NftForUdtPaymentCase, limit_order_action_failure_case,
    limit_order_nft_for_udt_case, limit_order_nft_for_udt_case_with,
};

use crate::framework::{
    cells::{TestCellOutput, live_input, normal_output, typed_output},
    cobuild::{CobuildMessageBuilder, OtxBuilder},
    contracts::{DeployedScript, cell_dep_for_script, deploy_data2_script},
    fixture::CobuildTestFixture,
    scripts::script_hash,
};

pub(crate) const CREATE_ORDER_TAG: u8 = 1;
pub(crate) const FILL_ORDER_TAG: u8 = 2;
#[cfg(not(test))]
pub(crate) const ORDER_ID: [u8; 32] = [1; 32];
const OFFERED_ASSET_ID: [u8; 32] = [3; 32];
const REQUESTED_ASSET_ID: [u8; 32] = [4; 32];
#[cfg(not(test))]
pub(crate) const NFT_TYPE_ARGS: [u8; 32] = [5; 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LimitOrderState {
    pub owner_lock_hash: [u8; 32],
    pub offered_nft_type_hash: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub min_requested_amount: u64,
}

pub fn order_data(order: LimitOrderState) -> Vec<u8> {
    let mut data = Vec::with_capacity(104);
    data.extend_from_slice(&order.owner_lock_hash);
    data.extend_from_slice(&order.offered_nft_type_hash);
    data.extend_from_slice(&order.requested_asset_id);
    data.extend_from_slice(&order.min_requested_amount.to_le_bytes());
    data
}

pub fn create_order_action_data(order: LimitOrderState) -> Vec<u8> {
    let mut data = Vec::with_capacity(105);
    data.push(CREATE_ORDER_TAG);
    data.extend_from_slice(&order.owner_lock_hash);
    data.extend_from_slice(&order.offered_nft_type_hash);
    data.extend_from_slice(&order.requested_asset_id);
    data.extend_from_slice(&order.min_requested_amount.to_le_bytes());
    data
}

pub fn settlement_data(asset_id: [u8; 32], amount: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(40);
    data.extend_from_slice(&asset_id);
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

pub fn limit_order_case(settlement_amount: u64) -> (CobuildTestFixture, TransactionView) {
    let mut fixture = CobuildTestFixture::new();

    let limit_order = fixture.deploy_limit_order();
    let always_success = fixture.deploy_always_success();
    let owner_lock = always_success.script.clone();

    let order_input = fixture
        .limit_order()
        .owner(owner_lock.clone())
        .offered_nft_type_hash(OFFERED_ASSET_ID)
        .requested_asset_id(REQUESTED_ASSET_ID)
        .min_requested_amount(30)
        .build_input(&limit_order.script);

    let settlement_output = LimitOrderBuilder::settlement_output(
        owner_lock,
        REQUESTED_ASSET_ID,
        settlement_amount,
        90_000_000_000,
    );

    let message = fixture
        .cobuild()
        .input_type_action(limit_order.script_hash)
        .limit_order_fill(REQUESTED_ASSET_ID, 30)
        .build();
    let otx = fixture
        .limit_order_append_settlement_otx()
        .message(message)
        .build_with_layout();

    let tx = fixture
        .tx()
        .cell_dep(cell_dep_for_script(&limit_order))
        .cell_dep(cell_dep_for_script(&always_success))
        .base_input(order_input)
        .append_output(settlement_output)
        .otx(otx)
        .build();

    (fixture, tx)
}

pub fn failed_txs_count() -> usize {
    let path = std::env::current_dir()
        .expect("current dir")
        .join("failed_txs");
    match std::fs::read_dir(path) {
        Ok(entries) => entries.count(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => panic!("read failed_txs: {error}"),
    }
}

pub trait LimitOrderCobuildMessageExt {
    fn limit_order_create(self, order: LimitOrderState) -> Self;
    fn limit_order_fill(self, requested_asset_id: [u8; 32], min_requested_amount: u64) -> Self;
}

impl LimitOrderCobuildMessageExt for CobuildMessageBuilder {
    fn limit_order_create(self, order: LimitOrderState) -> Self {
        self.action_data(create_order_action_data(order))
    }

    fn limit_order_fill(self, requested_asset_id: [u8; 32], min_requested_amount: u64) -> Self {
        let mut data = Vec::with_capacity(41);
        data.push(FILL_ORDER_TAG);
        data.extend_from_slice(&requested_asset_id);
        data.extend_from_slice(&min_requested_amount.to_le_bytes());
        self.action_data(data)
    }
}

pub trait LimitOrderFixtureExt {
    fn deploy_limit_order(&mut self) -> DeployedScript;
    fn limit_order(&mut self) -> LimitOrderBuilder<'_>;
    fn limit_order_append_settlement_otx(&self) -> OtxBuilder;
}

impl LimitOrderFixtureExt for CobuildTestFixture {
    fn deploy_limit_order(&mut self) -> DeployedScript {
        deploy_data2_script(self.context_mut(), "limit-order-type", Vec::new())
    }

    fn limit_order(&mut self) -> LimitOrderBuilder<'_> {
        LimitOrderBuilder::new(self.context_mut())
    }

    fn limit_order_append_settlement_otx(&self) -> OtxBuilder {
        OtxBuilder::new()
            .base_input_cells(1)
            .append_output_cells(1)
            .allow_append_outputs()
    }
}

pub struct LimitOrderBuilder<'a> {
    context: &'a mut Context,
    owner: Option<Script>,
    offered_nft_type_hash: [u8; 32],
    requested_asset_id: [u8; 32],
    min_requested_amount: u64,
    capacity: u64,
}

impl<'a> LimitOrderBuilder<'a> {
    fn new(context: &'a mut Context) -> Self {
        Self {
            context,
            owner: None,
            offered_nft_type_hash: [3; 32],
            requested_asset_id: [4; 32],
            min_requested_amount: 30,
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

    pub fn offered_asset_id(self, asset_id: [u8; 32]) -> Self {
        self.offered_nft_type_hash(asset_id)
    }

    pub fn requested_asset_id(mut self, asset_id: [u8; 32]) -> Self {
        self.requested_asset_id = asset_id;
        self
    }

    pub fn offered_remaining(self, _amount: u64) -> Self {
        self
    }

    pub fn min_requested_per_offered(mut self, price: u64) -> Self {
        self.min_requested_amount = price.saturating_mul(10);
        self
    }

    pub fn min_requested_amount(mut self, amount: u64) -> Self {
        self.min_requested_amount = amount;
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
            min_requested_amount: self.min_requested_amount,
        });
        live_input(self.context, output, data)
    }

    pub fn settlement_output(
        owner: Script,
        requested_asset_id: [u8; 32],
        amount: u64,
        capacity: u64,
    ) -> TestCellOutput {
        TestCellOutput::new(
            normal_output(owner, capacity),
            settlement_data(requested_asset_id, amount),
        )
    }
}
