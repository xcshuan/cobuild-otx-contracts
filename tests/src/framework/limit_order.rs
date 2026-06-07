use ckb_testtool::{
    ckb_types::packed::{CellInput, Script},
    context::Context,
};

use super::{
    cells::{TestCellOutput, live_input, normal_output, typed_output},
    cobuild::CobuildMessageBuilder,
    contracts::{DeployedScript, deploy_data2_script},
    fixture::CobuildTestFixture,
    scripts::script_hash,
};

const FILL_ORDER_TAG: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LimitOrderState {
    pub order_id: [u8; 32],
    pub owner_lock_hash: [u8; 32],
    pub offered_asset_id: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub offered_remaining: u64,
    pub min_requested_per_offered: u64,
    pub nonce: u64,
}

pub fn order_data(order: LimitOrderState) -> Vec<u8> {
    let mut data = Vec::with_capacity(152);
    data.extend_from_slice(&order.order_id);
    data.extend_from_slice(&order.owner_lock_hash);
    data.extend_from_slice(&order.offered_asset_id);
    data.extend_from_slice(&order.requested_asset_id);
    data.extend_from_slice(&order.offered_remaining.to_le_bytes());
    data.extend_from_slice(&order.min_requested_per_offered.to_le_bytes());
    data.extend_from_slice(&order.nonce.to_le_bytes());
    data
}

pub fn settlement_data(asset_id: [u8; 32], amount: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(40);
    data.extend_from_slice(&asset_id);
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

pub trait LimitOrderCobuildMessageExt {
    fn limit_order_fill(
        self,
        order_id: [u8; 32],
        requested_asset_id: [u8; 32],
        offered_amount: u64,
        requested_amount: u64,
    ) -> Self;
}

impl LimitOrderCobuildMessageExt for CobuildMessageBuilder {
    fn limit_order_fill(
        self,
        order_id: [u8; 32],
        requested_asset_id: [u8; 32],
        offered_amount: u64,
        requested_amount: u64,
    ) -> Self {
        let mut data = Vec::with_capacity(81);
        data.push(FILL_ORDER_TAG);
        data.extend_from_slice(&order_id);
        data.extend_from_slice(&requested_asset_id);
        data.extend_from_slice(&offered_amount.to_le_bytes());
        data.extend_from_slice(&requested_amount.to_le_bytes());
        self.action_data(data)
    }
}

pub trait LimitOrderFixtureExt {
    fn deploy_limit_order(&mut self) -> DeployedScript;
    fn limit_order(&mut self) -> LimitOrderBuilder<'_>;
}

impl LimitOrderFixtureExt for CobuildTestFixture {
    fn deploy_limit_order(&mut self) -> DeployedScript {
        deploy_data2_script(self.context_mut(), "limit-order", Vec::new())
    }

    fn limit_order(&mut self) -> LimitOrderBuilder<'_> {
        LimitOrderBuilder::new(self.context_mut())
    }
}

pub struct LimitOrderBuilder<'a> {
    context: &'a mut Context,
    owner: Option<Script>,
    order_id: [u8; 32],
    offered_asset_id: [u8; 32],
    requested_asset_id: [u8; 32],
    offered_remaining: u64,
    min_requested_per_offered: u64,
    nonce: u64,
    capacity: u64,
}

impl<'a> LimitOrderBuilder<'a> {
    fn new(context: &'a mut Context) -> Self {
        Self {
            context,
            owner: None,
            order_id: [1; 32],
            offered_asset_id: [3; 32],
            requested_asset_id: [4; 32],
            offered_remaining: 10,
            min_requested_per_offered: 3,
            nonce: 9,
            capacity: 100_000_000_000,
        }
    }

    pub fn owner(mut self, owner: Script) -> Self {
        self.owner = Some(owner);
        self
    }

    pub fn offered_asset_id(mut self, asset_id: [u8; 32]) -> Self {
        self.offered_asset_id = asset_id;
        self
    }

    pub fn requested_asset_id(mut self, asset_id: [u8; 32]) -> Self {
        self.requested_asset_id = asset_id;
        self
    }

    pub fn offered_remaining(mut self, amount: u64) -> Self {
        self.offered_remaining = amount;
        self
    }

    pub fn min_requested_per_offered(mut self, price: u64) -> Self {
        self.min_requested_per_offered = price;
        self
    }

    pub fn build_input(self, limit_order_type: &Script) -> CellInput {
        let owner = self.owner.expect("limit order owner lock");
        let owner_lock_hash = script_hash(&owner);
        let output = typed_output(owner, limit_order_type.clone(), self.capacity);
        let data = order_data(LimitOrderState {
            order_id: self.order_id,
            owner_lock_hash,
            offered_asset_id: self.offered_asset_id,
            requested_asset_id: self.requested_asset_id,
            offered_remaining: self.offered_remaining,
            min_requested_per_offered: self.min_requested_per_offered,
            nonce: self.nonce,
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
