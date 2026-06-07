use ckb_testtool::{
    ckb_types::{core::TransactionView, packed::Script},
    context::Context,
};

use super::{
    assertions,
    cells::{LimitOrderState, TestCellOutput, live_input, order_data, typed_output},
    cobuild::{CobuildMessageBuilder, OtxBuilder},
    contracts::{DeployedScript, deploy_always_success, deploy_data2_script},
    scripts::script_hash,
    tx::OtxTransactionBuilder,
};

pub struct CobuildTestFixture {
    context: Context,
}

impl CobuildTestFixture {
    pub fn new() -> Self {
        Self {
            context: Context::default(),
        }
    }

    pub fn context(&self) -> &Context {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    pub fn deploy_limit_order(&mut self) -> DeployedScript {
        deploy_data2_script(&mut self.context, "limit-order", Vec::new())
    }

    pub fn deploy_always_success(&mut self) -> DeployedScript {
        deploy_always_success(&mut self.context, Vec::new())
    }

    pub fn limit_order(&mut self) -> LimitOrderBuilder<'_> {
        LimitOrderBuilder::new(&mut self.context)
    }

    pub fn cobuild(&self) -> CobuildMessageBuilder {
        CobuildMessageBuilder::new()
    }

    pub fn otx(&self) -> OtxBuilder {
        OtxBuilder::new()
    }

    pub fn tx(&self) -> OtxTransactionBuilder {
        OtxTransactionBuilder::new()
    }

    pub fn assert_pass(&self, tx: &TransactionView) {
        assertions::assert_pass(&self.context, tx);
    }

    pub fn assert_type_script_exit(&self, tx: &TransactionView, input_index: usize, code: i8) {
        assertions::assert_type_script_exit(&self.context, tx, input_index, code);
    }
}

impl Default for CobuildTestFixture {
    fn default() -> Self {
        Self::new()
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

    pub fn build_input(
        self,
        limit_order_type: &Script,
    ) -> ckb_testtool::ckb_types::packed::CellInput {
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
            super::cells::normal_output(owner, capacity),
            super::cells::settlement_data(requested_asset_id, amount),
        )
    }
}
