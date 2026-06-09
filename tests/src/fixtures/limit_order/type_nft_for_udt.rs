use ckb_hash::new_blake2b;
use ckb_testtool::ckb_types::{
    bytes::Bytes,
    core::{ScriptHashType, TransactionView},
    packed::CellInput,
    prelude::*,
};

use crate::framework::{
    cells::{TestCellOutput, live_input, normal_output, typed_output},
    cobuild::empty_message,
    contracts::{DeployedScript, cell_dep_for_script, deploy_always_success, deploy_data2_script},
    fixture::CobuildTestFixture,
    scripts::script_hash,
};

use super::{
    LimitOrderCobuildMessageExt, LimitOrderFixtureExt, LimitOrderState, NFT_TYPE_ARGS, order_data,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NftForUdtPaymentCase {
    Valid,
    InsufficientUdt,
    WrongUdt,
    WrongOwner,
    TxLevelRemainderOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FillActionCase {
    TxLevelFillOrder,
    OutputTypeTarget,
    RequestedAssetMismatch,
    MinRequestedBelowRequired,
    NoRelatedAction,
    MultipleRelatedActions,
    OrderTypeOnlyInAppendInputRelation,
    PaymentInAnotherOtx,
    PaymentOutputOutOfRange,
    PaymentOutputWrongUdt,
    PaymentOutputWrongOwner,
    PaymentOutputInsufficient,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateOrderCase {
    Valid,
    MissingNftProxyOutput,
    WrongNftType,
    WrongProxyOrder,
    StateActionMismatch,
    InvalidTypeId,
    InputAndOutputGroupShape,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NftForUdtScenario {
    payment_case: NftForUdtPaymentCase,
    action_case: Option<FillActionCase>,
}

impl NftForUdtScenario {
    fn payment(case: NftForUdtPaymentCase) -> Self {
        Self {
            payment_case: case,
            action_case: None,
        }
    }

    fn action(case: FillActionCase) -> Self {
        Self {
            payment_case: NftForUdtPaymentCase::Valid,
            action_case: Some(case),
        }
    }
}

pub fn limit_order_nft_for_udt_case() -> (CobuildTestFixture, TransactionView) {
    limit_order_nft_for_udt_case_with(NftForUdtPaymentCase::Valid)
}

pub fn limit_order_nft_for_udt_case_with(
    case: NftForUdtPaymentCase,
) -> (CobuildTestFixture, TransactionView) {
    limit_order_nft_for_udt_scenario(NftForUdtScenario::payment(case))
}

pub fn limit_order_action_failure_case(
    case: FillActionCase,
) -> (CobuildTestFixture, TransactionView) {
    limit_order_nft_for_udt_scenario(NftForUdtScenario::action(case))
}

fn limit_order_nft_for_udt_scenario(
    scenario: NftForUdtScenario,
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
    let payment_udt = if scenario.payment_case == NftForUdtPaymentCase::WrongUdt
        || scenario.action_case == Some(FillActionCase::PaymentOutputWrongUdt)
    {
        wrong_udt.clone()
    } else {
        udt.clone()
    };
    let payment_lock = if scenario.payment_case == NftForUdtPaymentCase::WrongOwner
        || scenario.action_case == Some(FillActionCase::PaymentOutputWrongOwner)
    {
        wrong_owner_lock
    } else {
        owner_lock.clone()
    };
    let insufficient_append_payment = matches!(
        scenario.payment_case,
        NftForUdtPaymentCase::InsufficientUdt | NftForUdtPaymentCase::TxLevelRemainderOnly
    ) || matches!(
        scenario.action_case,
        Some(FillActionCase::PaymentInAnotherOtx | FillActionCase::PaymentOutputInsufficient)
    );
    let payment_amount = if insufficient_append_payment { 29 } else { 30 };
    let remainder_payment_output =
        if scenario.payment_case == NftForUdtPaymentCase::TxLevelRemainderOnly
            || scenario.action_case == Some(FillActionCase::PaymentOutputOutOfRange)
        {
            Some(TestCellOutput::new(
                typed_output(owner_lock.clone(), udt.script.clone(), 90_000_000_000),
                udt_amount_data(30),
            ))
        } else {
            None
        };
    let other_otx_payment_output =
        if scenario.action_case == Some(FillActionCase::PaymentInAnotherOtx) {
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
        .offered_nft_type_hash(nft.script_hash)
        .requested_asset_id(udt.script_hash)
        .min_requested_amount(30)
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
    let dummy_base_input = if scenario.action_case == Some(FillActionCase::PaymentInAnotherOtx) {
        Some(live_input(
            fixture.context_mut(),
            normal_output(always_success.script.clone(), 100_000_000_000),
            Vec::new(),
        ))
    } else {
        None
    };

    let action_target = match scenario.action_case {
        Some(FillActionCase::OutputTypeTarget) => {
            fixture.cobuild().output_type_action(nft.script_hash)
        }
        _ => fixture.cobuild().input_type_action(limit_order.script_hash),
    };
    let action_requested_asset = match scenario.action_case {
        Some(FillActionCase::RequestedAssetMismatch) => wrong_udt.script_hash,
        _ => udt.script_hash,
    };
    let action_requested_amount = match scenario.action_case {
        Some(FillActionCase::MinRequestedBelowRequired) => 29,
        _ => 30,
    };
    let payment_output_index = match scenario.action_case {
        Some(FillActionCase::PaymentInAnotherOtx | FillActionCase::PaymentOutputOutOfRange) => 2,
        _ => 1,
    };
    let fill_order_message = LimitOrderCobuildMessageExt::limit_order_fill(
        action_target,
        action_requested_asset,
        action_requested_amount,
        payment_output_index,
    )
    .build();
    let otx_message = if scenario.action_case == Some(FillActionCase::TxLevelFillOrder) {
        empty_message()
    } else {
        fill_order_message.clone()
    };
    let otx = fixture
        .otx()
        .base_input_cells(2)
        .base_output_cells(1)
        .append_input_cells(1)
        .append_output_cells(1)
        .allow_append_inputs()
        .allow_append_outputs()
        .message(otx_message)
        .build_with_layout();
    let other_otx = if scenario.action_case == Some(FillActionCase::PaymentInAnotherOtx) {
        Some(
            fixture
                .otx()
                .base_input_cells(1)
                .append_output_cells(1)
                .allow_append_outputs()
                .message(empty_message())
                .build_with_layout(),
        )
    } else {
        None
    };

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
    if let Some(input) = dummy_base_input {
        tx = tx.base_input(input);
    }
    if let Some(output) = remainder_payment_output {
        tx = tx.remainder_output(output);
    }
    if let Some(output) = other_otx_payment_output {
        tx = tx.append_output(output);
    }
    if scenario.action_case == Some(FillActionCase::TxLevelFillOrder) {
        tx = tx.tx_level_message(fill_order_message);
    }
    tx = tx.otx(otx);
    if let Some(other_otx) = other_otx {
        tx = tx.otx(other_otx);
    }
    let tx = tx.build();

    (fixture, tx)
}

pub fn limit_order_create_nft_order_case() -> (CobuildTestFixture, TransactionView) {
    limit_order_create_nft_order_case_with(CreateOrderCase::Valid)
}

pub fn limit_order_create_nft_order_case_with(
    case: CreateOrderCase,
) -> (CobuildTestFixture, TransactionView) {
    let mut fixture = CobuildTestFixture::new();

    let limit_order_code = fixture.deploy_limit_order();
    let always_success = fixture.deploy_always_success();
    let owner_lock = always_success.script.clone();
    let funding_input = live_input(
        fixture.context_mut(),
        normal_output(owner_lock.clone(), 200_000_000_000),
        Vec::new(),
    );
    let nft_type_id = type_id_args(&funding_input, 1);
    let nft = deploy_test_nft(&mut fixture, nft_type_id);
    let output_nft = if case == CreateOrderCase::WrongNftType {
        deploy_test_nft(&mut fixture, type_id_args(&funding_input, 2))
    } else {
        nft.clone()
    };
    let udt = deploy_test_udt_with_owner(&mut fixture, script_hash(&always_success.script));

    let computed_order_type_id = type_id_args(&funding_input, 0);
    let order_type_id = if case == CreateOrderCase::InvalidTypeId {
        [9; 32]
    } else {
        computed_order_type_id
    };
    let order_type = fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&order_type_id),
        )
        .expect("build order type-id script");
    let order_type_hash = script_hash(&order_type);
    let proxy_owner_type_hash = if case == CreateOrderCase::WrongProxyOrder {
        [8; 32]
    } else {
        order_type_hash
    };
    let proxy_lock = deploy_input_type_proxy_lock(&mut fixture, proxy_owner_type_hash);
    let order_state = LimitOrderState {
        owner_lock_hash: script_hash(&owner_lock),
        offered_nft_type_hash: nft.script_hash,
        requested_asset_id: udt.script_hash,
        min_requested_amount: 30,
    };
    let action_state = LimitOrderState {
        min_requested_amount: if case == CreateOrderCase::StateActionMismatch {
            31
        } else {
            order_state.min_requested_amount
        },
        ..order_state
    };
    let order_output = TestCellOutput::new(
        typed_output(owner_lock.clone(), order_type.clone(), 100_000_000_000),
        order_data(order_state),
    );
    let wrong_nft_padding_output = if case == CreateOrderCase::WrongNftType {
        Some(TestCellOutput::new(
            normal_output(always_success.script.clone(), 10_000_000_000),
            Vec::new(),
        ))
    } else {
        None
    };
    let order_input = if case == CreateOrderCase::InputAndOutputGroupShape {
        Some(live_input(
            fixture.context_mut(),
            typed_output(owner_lock, order_type.clone(), 100_000_000_000),
            order_data(order_state),
        ))
    } else {
        None
    };
    let nft_output = TestCellOutput::new(
        typed_output(
            proxy_lock.script.clone(),
            output_nft.script.clone(),
            90_000_000_000,
        ),
        nft_data(b"order-nft", [1, 2, 3, 4], 1_717_171_717),
    );
    let message = fixture
        .cobuild()
        .output_type_action(order_type_hash)
        .limit_order_create(action_state)
        .build();
    let mut tx = fixture
        .tx()
        .allow_no_otx()
        .cell_dep(cell_dep_for_script(&limit_order_code))
        .cell_dep(cell_dep_for_script(&always_success))
        .cell_dep(cell_dep_for_script(&proxy_lock))
        .cell_dep(cell_dep_for_script(&nft))
        .cell_dep(cell_dep_for_script(&output_nft))
        .cell_dep(cell_dep_for_script(&udt));
    if let Some(order_input) = order_input {
        tx = tx.base_input(order_input).base_input(funding_input);
    } else {
        tx = tx.base_input(funding_input);
    }
    tx = tx.base_output(order_output);
    if let Some(output) = wrong_nft_padding_output {
        tx = tx.base_output(output);
    }
    if !matches!(
        case,
        CreateOrderCase::MissingNftProxyOutput | CreateOrderCase::InputAndOutputGroupShape
    ) {
        tx = tx.base_output(nft_output);
    }
    let tx = tx.tx_level_message(message).build();

    (fixture, tx)
}

fn type_id_args(first_input: &CellInput, output_index: u64) -> [u8; 32] {
    let mut blake2b = new_blake2b();
    blake2b.update(first_input.as_slice());
    blake2b.update(&output_index.to_le_bytes());
    let mut out = [0u8; 32];
    blake2b.finalize(&mut out);
    out
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
