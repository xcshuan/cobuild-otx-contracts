use ckb_hash::new_blake2b;
use ckb_testtool::ckb_types::{
    bytes::Bytes,
    core::{ScriptHashType, TransactionView},
    packed::CellInput,
    prelude::*,
};

use crate::fixtures::common::{
    assets::{nft_data, udt_amount_data},
    contracts::{
        deploy_always_success, deploy_input_type_proxy_lock, deploy_test_nft, deploy_test_udt,
        deploy_wrong_owner_lock,
    },
};
use crate::framework::{
    cells::{TestCellOutput, live_input, normal_output, typed_output},
    cobuild::empty_message,
    contracts::cell_dep_for_script,
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
    NoRelatedAction,
    MultipleRelatedActions,
    OrderTypeOnlyInAppendInputRelation,
    PaymentInAnotherOtx,
    PaymentOutputOutOfRange,
    PaymentOutputWrongUdt,
    PaymentOutputWrongOwner,
    PaymentOutputInsufficient,
    MissingBuyerNftOutput,
    BuyerNftWrongLock,
    BuyerNftWrongType,
    TwoTypeOrdersReusePaymentOutput,
    TwoTypeOrdersUseDistinctPaymentOutputs,
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

pub fn limit_order_type_otx_with_sighash_all_fill_case() -> (CobuildTestFixture, TransactionView) {
    let mut fixture = CobuildTestFixture::new();

    let limit_order = fixture.deploy_limit_order();
    let always_success = deploy_always_success(fixture.context_mut(), Vec::new());
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let proxy_lock = deploy_input_type_proxy_lock(fixture.context_mut(), limit_order.script_hash);
    let nft = deploy_test_nft(fixture.context_mut(), NFT_TYPE_ARGS);
    let udt = deploy_test_udt(fixture.context_mut(), issuer_lock_hash);

    let nft_payload = nft_data(b"type-sighash-fill", [1, 2, 3, 4], 1_717_171_719);
    let order_input = fixture
        .limit_order()
        .owner(owner_lock.clone())
        .offered_nft_type_hash(nft.script_hash)
        .requested_asset_id(udt.script_hash)
        .requested_amount(30)
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
    let buyer_udt_input = live_input(
        fixture.context_mut(),
        typed_output(buyer_lock.clone(), udt.script.clone(), 100_000_000_000),
        udt_amount_data(30),
    );
    let nft_output = TestCellOutput::new(
        typed_output(buyer_lock.clone(), nft.script.clone(), 90_000_000_000),
        nft_payload,
    );
    let payment_output = TestCellOutput::new(
        typed_output(owner_lock, udt.script.clone(), 90_000_000_000),
        udt_amount_data(30),
    );
    let fill_order_message = LimitOrderCobuildMessageExt::limit_order_fill(
        fixture.cobuild().input_type_action(limit_order.script_hash),
        1,
        script_hash(&buyer_lock),
    )
    .build();
    let otx = fixture
        .otx()
        .base_input_cells(2)
        .base_output_cells(1)
        .append_output_cells(1)
        .allow_append_outputs()
        .message(fill_order_message)
        .build_with_layout();
    let tx = fixture
        .tx()
        .cell_dep(cell_dep_for_script(&limit_order))
        .cell_dep(cell_dep_for_script(&always_success))
        .cell_dep(cell_dep_for_script(&proxy_lock))
        .cell_dep(cell_dep_for_script(&nft))
        .cell_dep(cell_dep_for_script(&udt))
        .base_input(order_input)
        .base_input(nft_input)
        .append_input(buyer_udt_input)
        .base_output(nft_output)
        .append_output(payment_output)
        .tx_level_message(empty_message())
        .otx(otx)
        .build();

    (fixture, tx)
}

pub fn limit_order_action_failure_case(
    case: FillActionCase,
) -> (CobuildTestFixture, TransactionView) {
    if matches!(
        case,
        FillActionCase::TwoTypeOrdersReusePaymentOutput
            | FillActionCase::TwoTypeOrdersUseDistinctPaymentOutputs
    ) {
        return limit_order_two_type_orders_case(case);
    }

    limit_order_nft_for_udt_scenario(NftForUdtScenario::action(case))
}

fn limit_order_two_type_orders_case(case: FillActionCase) -> (CobuildTestFixture, TransactionView) {
    let mut fixture = CobuildTestFixture::new();

    let limit_order_code = fixture.deploy_limit_order();
    let always_success = deploy_always_success(fixture.context_mut(), Vec::new());
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let nft_a = deploy_test_nft(fixture.context_mut(), [0x51; 32]);
    let nft_b = deploy_test_nft(fixture.context_mut(), [0x52; 32]);
    let udt = deploy_test_udt(fixture.context_mut(), issuer_lock_hash);
    let order_type_a = fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&[0x61; 32]),
        )
        .expect("build first order type");
    let order_type_b = fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&[0x62; 32]),
        )
        .expect("build second order type");
    let order_type_hash_a = script_hash(&order_type_a);
    let order_type_hash_b = script_hash(&order_type_b);
    let proxy_lock_a = deploy_input_type_proxy_lock(fixture.context_mut(), order_type_hash_a);
    let proxy_lock_b = deploy_input_type_proxy_lock(fixture.context_mut(), order_type_hash_b);

    let order_input_a = fixture
        .limit_order()
        .owner(owner_lock.clone())
        .offered_nft_type_hash(nft_a.script_hash)
        .requested_asset_id(udt.script_hash)
        .requested_amount(30)
        .build_input(&order_type_a);
    let order_input_b = fixture
        .limit_order()
        .owner(owner_lock.clone())
        .offered_nft_type_hash(nft_b.script_hash)
        .requested_asset_id(udt.script_hash)
        .requested_amount(30)
        .build_input(&order_type_b);
    let nft_payload_a = nft_data(b"type-order-a", [1, 2, 3, 4], 1_717_171_717);
    let nft_payload_b = nft_data(b"type-order-b", [5, 6, 7, 8], 1_717_171_718);
    let nft_input_a = live_input(
        fixture.context_mut(),
        typed_output(
            proxy_lock_a.script.clone(),
            nft_a.script.clone(),
            100_000_000_000,
        ),
        nft_payload_a.clone(),
    );
    let nft_input_b = live_input(
        fixture.context_mut(),
        typed_output(
            proxy_lock_b.script.clone(),
            nft_b.script.clone(),
            100_000_000_000,
        ),
        nft_payload_b.clone(),
    );
    let udt_input = live_input(
        fixture.context_mut(),
        typed_output(buyer_lock.clone(), udt.script.clone(), 100_000_000_000),
        udt_amount_data(60),
    );
    let nft_output_a = TestCellOutput::new(
        typed_output(buyer_lock.clone(), nft_a.script.clone(), 90_000_000_000),
        nft_payload_a,
    );
    let nft_output_b = TestCellOutput::new(
        typed_output(buyer_lock.clone(), nft_b.script.clone(), 90_000_000_000),
        nft_payload_b,
    );
    let payment_output_a = TestCellOutput::new(
        typed_output(owner_lock.clone(), udt.script.clone(), 90_000_000_000),
        udt_amount_data(30),
    );
    let payment_output_b = TestCellOutput::new(
        typed_output(owner_lock, udt.script.clone(), 90_000_000_000),
        udt_amount_data(30),
    );
    let second_payment_index = if case == FillActionCase::TwoTypeOrdersReusePaymentOutput {
        2
    } else {
        3
    };
    let message = fixture
        .cobuild()
        .push_action(
            1,
            order_type_hash_a,
            fill_action_data(2, script_hash(&buyer_lock)),
        )
        .push_action(
            1,
            order_type_hash_b,
            fill_action_data(second_payment_index, script_hash(&buyer_lock)),
        )
        .build();
    let otx = fixture
        .otx()
        .base_input_cells(4)
        .base_output_cells(2)
        .append_input_cells(1)
        .append_output_cells(2)
        .allow_append_inputs()
        .allow_append_outputs()
        .message(message)
        .build_with_layout();

    let tx = fixture
        .tx()
        .cell_dep(cell_dep_for_script(&limit_order_code))
        .cell_dep(cell_dep_for_script(&always_success))
        .cell_dep(cell_dep_for_script(&proxy_lock_a))
        .cell_dep(cell_dep_for_script(&proxy_lock_b))
        .cell_dep(cell_dep_for_script(&nft_a))
        .cell_dep(cell_dep_for_script(&nft_b))
        .cell_dep(cell_dep_for_script(&udt))
        .base_input(order_input_a)
        .base_input(nft_input_a)
        .base_input(order_input_b)
        .base_input(nft_input_b)
        .append_input(udt_input)
        .base_output(nft_output_a)
        .base_output(nft_output_b)
        .append_output(payment_output_a)
        .append_output(payment_output_b)
        .otx(otx)
        .build();

    (fixture, tx)
}

fn limit_order_nft_for_udt_scenario(
    scenario: NftForUdtScenario,
) -> (CobuildTestFixture, TransactionView) {
    let mut fixture = CobuildTestFixture::new();

    let limit_order = fixture.deploy_limit_order();
    let always_success = deploy_always_success(fixture.context_mut(), Vec::new());
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let wrong_owner_lock = deploy_wrong_owner_lock(fixture.context_mut()).script;
    let wrong_buyer_lock = deploy_wrong_owner_lock(fixture.context_mut()).script;
    let proxy_lock = deploy_input_type_proxy_lock(fixture.context_mut(), limit_order.script_hash);
    let nft = deploy_test_nft(fixture.context_mut(), NFT_TYPE_ARGS);
    let wrong_nft = deploy_test_nft(fixture.context_mut(), [0x66; 32]);
    let udt = deploy_test_udt(fixture.context_mut(), issuer_lock_hash);
    let wrong_udt = deploy_test_udt(fixture.context_mut(), [9; 32]);
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
    let remainder_payment_output = if scenario.payment_case
        == NftForUdtPaymentCase::TxLevelRemainderOnly
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
        .requested_amount(30)
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
    let nft_output = match scenario.action_case {
        Some(FillActionCase::MissingBuyerNftOutput) => TestCellOutput::new(
            normal_output(always_success.script.clone(), 90_000_000_000),
            Vec::new(),
        ),
        Some(FillActionCase::BuyerNftWrongLock) => TestCellOutput::new(
            typed_output(wrong_buyer_lock, nft.script.clone(), 90_000_000_000),
            nft_payload,
        ),
        Some(FillActionCase::BuyerNftWrongType) => TestCellOutput::new(
            typed_output(buyer_lock.clone(), wrong_nft.script.clone(), 90_000_000_000),
            nft_payload,
        ),
        _ => TestCellOutput::new(
            typed_output(buyer_lock.clone(), nft.script.clone(), 90_000_000_000),
            nft_payload,
        ),
    };
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
    let payment_output_index = match scenario.action_case {
        Some(FillActionCase::PaymentInAnotherOtx | FillActionCase::PaymentOutputOutOfRange) => 2,
        _ => 1,
    };
    let fill_order_message = LimitOrderCobuildMessageExt::limit_order_fill(
        action_target,
        payment_output_index,
        script_hash(&buyer_lock),
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
        .cell_dep(cell_dep_for_script(&wrong_nft))
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
    let always_success = deploy_always_success(fixture.context_mut(), Vec::new());
    let owner_lock = always_success.script.clone();
    let funding_input = live_input(
        fixture.context_mut(),
        normal_output(owner_lock.clone(), 200_000_000_000),
        Vec::new(),
    );
    let nft_type_id = type_id_args(&funding_input, 1);
    let nft = deploy_test_nft(fixture.context_mut(), nft_type_id);
    let output_nft = if case == CreateOrderCase::WrongNftType {
        deploy_test_nft(fixture.context_mut(), type_id_args(&funding_input, 2))
    } else {
        nft.clone()
    };
    let udt = deploy_test_udt(fixture.context_mut(), script_hash(&always_success.script));

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
    let proxy_lock = deploy_input_type_proxy_lock(fixture.context_mut(), proxy_owner_type_hash);
    let order_state = LimitOrderState {
        owner_lock_hash: script_hash(&owner_lock),
        offered_nft_type_hash: nft.script_hash,
        requested_asset_id: udt.script_hash,
        requested_amount: 30,
    };
    let action_state = LimitOrderState {
        requested_amount: if case == CreateOrderCase::StateActionMismatch {
            31
        } else {
            order_state.requested_amount
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

fn fill_action_data(payment_output_index: u32, buyer_lock_hash: [u8; 32]) -> Vec<u8> {
    let mut data = Vec::with_capacity(37);
    data.push(super::FILL_ORDER_TAG);
    data.extend_from_slice(&payment_output_index.to_le_bytes());
    data.extend_from_slice(&buyer_lock_hash);
    data
}
