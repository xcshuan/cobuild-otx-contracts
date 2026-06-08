use ckb_testtool::{
    ckb_types::{
        core::TransactionView,
        packed::{CellInput, Script},
    },
    context::Context,
};

use crate::framework::{
    cells::{TestCellOutput, live_input, normal_output, typed_output},
    cobuild::{CobuildMessageBuilder, OtxBuilder},
    contracts::{DeployedScript, cell_dep_for_script, deploy_always_success, deploy_data2_script},
    fixture::CobuildTestFixture,
    scripts::script_hash,
};

const FILL_ORDER_TAG: u8 = 1;
const ORDER_ID: [u8; 32] = [1; 32];
const OFFERED_ASSET_ID: [u8; 32] = [3; 32];
const REQUESTED_ASSET_ID: [u8; 32] = [4; 32];
const NFT_TYPE_ARGS: [u8; 32] = [5; 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NftForUdtPaymentCase {
    Valid,
    InsufficientUdt,
    WrongUdt,
    WrongOwner,
    TxLevelRemainderOnly,
}

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

pub fn limit_order_case(settlement_amount: u64) -> (CobuildTestFixture, TransactionView) {
    let mut fixture = CobuildTestFixture::new();

    let limit_order = fixture.deploy_limit_order();
    let always_success = fixture.deploy_always_success();
    let owner_lock = always_success.script.clone();

    let order_input = fixture
        .limit_order()
        .owner(owner_lock.clone())
        .offered_asset_id(OFFERED_ASSET_ID)
        .requested_asset_id(REQUESTED_ASSET_ID)
        .offered_remaining(10)
        .min_requested_per_offered(3)
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
        .limit_order_fill(ORDER_ID, REQUESTED_ASSET_ID, 10, 30)
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

pub fn limit_order_nft_for_udt_case() -> (CobuildTestFixture, TransactionView) {
    limit_order_nft_for_udt_case_with(NftForUdtPaymentCase::Valid)
}

pub fn limit_order_nft_for_udt_case_with(
    case: NftForUdtPaymentCase,
) -> (CobuildTestFixture, TransactionView) {
    let mut fixture = CobuildTestFixture::new();

    let limit_order = fixture.deploy_limit_order();
    let always_success = fixture.deploy_always_success();
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let wrong_owner_lock = deploy_wrong_owner_lock(&mut fixture).script;
    let proxy_lock = deploy_input_type_proxy_lock(&mut fixture, limit_order.script_hash);
    let nft = deploy_test_nft(&mut fixture, NFT_TYPE_ARGS);
    let udt = deploy_test_udt_with_owner(&mut fixture, issuer_lock_hash);
    let wrong_udt = deploy_test_udt_with_owner(&mut fixture, [9; 32]);
    let payment_udt = if case == NftForUdtPaymentCase::WrongUdt {
        wrong_udt.clone()
    } else {
        udt.clone()
    };
    let payment_lock = if case == NftForUdtPaymentCase::WrongOwner {
        wrong_owner_lock
    } else {
        owner_lock.clone()
    };
    let payment_amount = match case {
        NftForUdtPaymentCase::InsufficientUdt | NftForUdtPaymentCase::TxLevelRemainderOnly => 29,
        NftForUdtPaymentCase::Valid
        | NftForUdtPaymentCase::WrongUdt
        | NftForUdtPaymentCase::WrongOwner => 30,
    };
    let remainder_payment_output = if case == NftForUdtPaymentCase::TxLevelRemainderOnly {
        Some(TestCellOutput::new(
            typed_output(owner_lock.clone(), udt.script.clone(), 90_000_000_000),
            udt_amount_data(1),
        ))
    } else {
        None
    };

    let nft_payload = nft_data(b"order-nft", [1, 2, 3, 4], 1_717_171_717);
    let order_input = fixture
        .limit_order()
        .owner(owner_lock.clone())
        .offered_asset_id(nft.script_hash)
        .requested_asset_id(udt.script_hash)
        .offered_remaining(10)
        .min_requested_per_offered(3)
        .build_input(&limit_order.script);
    let nft_input = live_input(
        fixture.context_mut(),
        typed_output(
            proxy_lock.script.clone(),
            nft.script.clone(),
            100_000_000_000,
        ),
        nft_payload.clone(),
    );
    let udt_input = live_input(
        fixture.context_mut(),
        typed_output(
            buyer_lock.clone(),
            payment_udt.script.clone(),
            100_000_000_000,
        ),
        udt_amount_data(30),
    );
    let nft_output = TestCellOutput::new(
        typed_output(buyer_lock, nft.script.clone(), 90_000_000_000),
        nft_payload,
    );
    let udt_payment_output = TestCellOutput::new(
        typed_output(payment_lock, payment_udt.script.clone(), 90_000_000_000),
        udt_amount_data(payment_amount),
    );

    let message = fixture
        .cobuild()
        .input_type_action(limit_order.script_hash)
        .limit_order_fill(ORDER_ID, udt.script_hash, 10, 30)
        .build();
    let otx = fixture
        .otx()
        .base_input_cells(2)
        .base_output_cells(1)
        .append_input_cells(1)
        .append_output_cells(1)
        .allow_append_inputs()
        .allow_append_outputs()
        .message(message)
        .build_with_layout();

    let mut tx = fixture
        .tx()
        .cell_dep(cell_dep_for_script(&limit_order))
        .cell_dep(cell_dep_for_script(&always_success))
        .cell_dep(cell_dep_for_script(&proxy_lock))
        .cell_dep(cell_dep_for_script(&nft))
        .cell_dep(cell_dep_for_script(&udt))
        .cell_dep(cell_dep_for_script(&wrong_udt))
        .base_input(order_input)
        .base_input(nft_input)
        .append_input(udt_input)
        .base_output(nft_output)
        .append_output(udt_payment_output);
    if let Some(output) = remainder_payment_output {
        tx = tx.remainder_output(output);
    }
    let tx = tx.otx(otx).build();

    (fixture, tx)
}

fn deploy_wrong_owner_lock(fixture: &mut CobuildTestFixture) -> DeployedScript {
    deploy_always_success(fixture.context_mut(), b"wrong-owner".to_vec())
}

fn deploy_test_udt_with_owner(
    fixture: &mut CobuildTestFixture,
    owner_lock_hash: [u8; 32],
) -> DeployedScript {
    deploy_data2_script(fixture.context_mut(), "test-udt", owner_lock_hash.to_vec())
}

fn deploy_test_nft(fixture: &mut CobuildTestFixture, args: [u8; 32]) -> DeployedScript {
    deploy_data2_script(fixture.context_mut(), "test-nft", args.to_vec())
}

fn deploy_input_type_proxy_lock(
    fixture: &mut CobuildTestFixture,
    owner_type_hash: [u8; 32],
) -> DeployedScript {
    deploy_data2_script(
        fixture.context_mut(),
        "input-type-proxy-lock",
        owner_type_hash.to_vec(),
    )
}

fn nft_data(name: &[u8], attributes: [u8; 4], created_at: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + name.len() + 4 + 8);
    data.push(name.len() as u8);
    data.extend_from_slice(name);
    data.extend_from_slice(&attributes);
    data.extend_from_slice(&created_at.to_le_bytes());
    data
}

fn udt_amount_data(amount: u128) -> Vec<u8> {
    amount.to_le_bytes().to_vec()
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
