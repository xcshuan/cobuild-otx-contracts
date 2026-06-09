use ckb_testtool::ckb_types::{
    bytes::Bytes,
    core::{ScriptHashType, TransactionView},
};

use crate::framework::{
    cells::{TestCellOutput, live_input, normal_output, typed_output},
    cobuild::{empty_message, seal_pair},
    contracts::{DeployedScript, cell_dep_for_script, deploy_always_success, deploy_data2_script},
    fixture::CobuildTestFixture,
    scripts::script_hash,
};

use super::{FILL_ORDER_TAG, LimitOrderFixtureExt, NFT_TYPE_ARGS};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitOrderLockFillCase {
    Valid,
    MalformedArgs,
    WrongNftType,
    TxLevelFillOrder,
    WrongActionTarget,
    OrderInputInAppendScope,
    RequestedAssetMismatch,
    MinRequestedBelowRequired,
    InsufficientUdt,
    WrongUdt,
    WrongOwner,
    TxLevelRemainderOnly,
    PaymentInAnotherOtx,
    PaymentOutputOutOfRange,
    PaymentOutputWrongUdt,
    PaymentOutputWrongOwner,
    PaymentOutputInsufficient,
    TwoLockOrdersReusePaymentOutput,
    TwoLockOrdersUseDistinctPaymentOutputs,
    UnknownActionTag,
    MalformedAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LockOrder {
    owner_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
    requested_asset_id: [u8; 32],
    min_requested_amount: u64,
}

pub fn limit_order_lock_nft_for_udt_case() -> (CobuildTestFixture, TransactionView) {
    limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::Valid)
}

pub fn mixed_limit_order_type_lock_duplicate_payment_case() -> (CobuildTestFixture, TransactionView)
{
    let mut fixture = CobuildTestFixture::new();
    let limit_order_type = fixture.deploy_limit_order();
    let limit_order_lock_code =
        deploy_data2_script(fixture.context_mut(), "limit-order-lock", Vec::new());
    let always_success = fixture.deploy_always_success();
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let proxy_lock = deploy_input_type_proxy_lock(&mut fixture, limit_order_type.script_hash);
    let nft = deploy_test_nft(&mut fixture, NFT_TYPE_ARGS);
    let lock_nft = deploy_test_nft(&mut fixture, [0x73; 32]);
    let udt = deploy_test_udt_with_owner(&mut fixture, issuer_lock_hash);

    let type_order_input = fixture
        .limit_order()
        .owner(owner_lock.clone())
        .offered_nft_type_hash(nft.script_hash)
        .requested_asset_id(udt.script_hash)
        .min_requested_amount(30)
        .build_input(&limit_order_type.script);

    let lock_order = LockOrder {
        owner_lock_hash: script_hash(&owner_lock),
        offered_nft_type_hash: lock_nft.script_hash,
        requested_asset_id: udt.script_hash,
        min_requested_amount: 30,
    };
    let order_lock = fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_lock_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&lock_args(lock_order)),
        )
        .expect("build mixed limit order lock");
    let order_lock_hash = script_hash(&order_lock);

    let type_nft_payload = nft_data(b"mixed-type-order", [1, 2, 3, 4], 1_717_171_717);
    let lock_nft_payload = nft_data(b"mixed-lock-order", [5, 6, 7, 8], 1_717_171_718);
    let type_nft_input = live_input(
        fixture.context_mut(),
        typed_output(
            proxy_lock.script.clone(),
            nft.script.clone(),
            100_000_000_000,
        ),
        type_nft_payload.clone(),
    );
    let lock_nft_input = live_input(
        fixture.context_mut(),
        typed_output(order_lock, lock_nft.script.clone(), 100_000_000_000),
        lock_nft_payload.clone(),
    );
    let udt_input = live_input(
        fixture.context_mut(),
        typed_output(buyer_lock.clone(), udt.script.clone(), 100_000_000_000),
        udt_amount_data(60),
    );
    let type_nft_output = TestCellOutput::new(
        typed_output(buyer_lock.clone(), nft.script.clone(), 90_000_000_000),
        type_nft_payload,
    );
    let lock_nft_output = TestCellOutput::new(
        typed_output(buyer_lock, lock_nft.script.clone(), 90_000_000_000),
        lock_nft_payload,
    );
    let payment_output = TestCellOutput::new(
        typed_output(owner_lock, udt.script.clone(), 90_000_000_000),
        udt_amount_data(30),
    );
    let shared_payment_output_index = 2u32;
    let message = fixture
        .cobuild()
        .push_action(
            1,
            limit_order_type.script_hash,
            fill_action_data(udt.script_hash, 30, shared_payment_output_index),
        )
        .push_action(
            0,
            order_lock_hash,
            fill_action_data(udt.script_hash, 30, shared_payment_output_index),
        )
        .build();
    let otx = fixture
        .otx()
        .base_input_cells(3)
        .base_output_cells(2)
        .append_input_cells(1)
        .append_output_cells(1)
        .allow_append_inputs()
        .allow_append_outputs()
        .message(message)
        .seals(vec![seal_pair(order_lock_hash, 0, Vec::new())])
        .build_with_layout();

    let tx = fixture
        .tx()
        .cell_dep(cell_dep_for_script(&limit_order_type))
        .cell_dep(cell_dep_for_script(&limit_order_lock_code))
        .cell_dep(cell_dep_for_script(&always_success))
        .cell_dep(cell_dep_for_script(&proxy_lock))
        .cell_dep(cell_dep_for_script(&nft))
        .cell_dep(cell_dep_for_script(&lock_nft))
        .cell_dep(cell_dep_for_script(&udt))
        .base_input(type_order_input)
        .base_input(type_nft_input)
        .base_input(lock_nft_input)
        .append_input(udt_input)
        .base_output(type_nft_output)
        .base_output(lock_nft_output)
        .append_output(payment_output)
        .otx(otx)
        .build();

    (fixture, tx)
}

pub fn limit_order_lock_nft_for_udt_case_with(
    case: LimitOrderLockFillCase,
) -> (CobuildTestFixture, TransactionView) {
    if matches!(
        case,
        LimitOrderLockFillCase::TwoLockOrdersReusePaymentOutput
            | LimitOrderLockFillCase::TwoLockOrdersUseDistinctPaymentOutputs
    ) {
        return limit_order_two_lock_orders_case(case);
    }

    let mut fixture = CobuildTestFixture::new();
    let limit_order_lock_code =
        deploy_data2_script(fixture.context_mut(), "limit-order-lock", Vec::new());
    let always_success = fixture.deploy_always_success();
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let wrong_owner = deploy_wrong_owner_lock(&mut fixture);
    let wrong_owner_lock = wrong_owner.script.clone();
    let nft = deploy_test_nft(&mut fixture, NFT_TYPE_ARGS);
    let wrong_nft = deploy_test_nft(&mut fixture, [6; 32]);
    let udt = deploy_test_udt_with_owner(&mut fixture, issuer_lock_hash);
    let wrong_udt = deploy_test_udt_with_owner(&mut fixture, [9; 32]);
    let input_nft = if case == LimitOrderLockFillCase::WrongNftType {
        wrong_nft.clone()
    } else {
        nft.clone()
    };
    let payment_udt = if matches!(
        case,
        LimitOrderLockFillCase::WrongUdt | LimitOrderLockFillCase::PaymentOutputWrongUdt
    ) {
        wrong_udt.clone()
    } else {
        udt.clone()
    };
    let payment_lock = if matches!(
        case,
        LimitOrderLockFillCase::WrongOwner | LimitOrderLockFillCase::PaymentOutputWrongOwner
    ) {
        wrong_owner_lock
    } else {
        owner_lock.clone()
    };
    let insufficient_append_payment = matches!(
        case,
        LimitOrderLockFillCase::InsufficientUdt
            | LimitOrderLockFillCase::TxLevelRemainderOnly
            | LimitOrderLockFillCase::PaymentInAnotherOtx
            | LimitOrderLockFillCase::PaymentOutputInsufficient
    );
    let payment_amount = if insufficient_append_payment { 29 } else { 30 };

    let order = LockOrder {
        owner_lock_hash: script_hash(&owner_lock),
        offered_nft_type_hash: nft.script_hash,
        requested_asset_id: udt.script_hash,
        min_requested_amount: 30,
    };
    let mut order_lock_args = lock_args(order);
    if case == LimitOrderLockFillCase::MalformedArgs {
        order_lock_args.pop();
    }
    let order_lock = fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_lock_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&order_lock_args),
        )
        .expect("build limit order lock");
    let order_lock_hash = script_hash(&order_lock);

    let nft_payload = nft_data(b"lock-order-nft", [1, 2, 3, 4], 1_717_171_717);
    let nft_input = live_input(
        fixture.context_mut(),
        typed_output(
            order_lock.clone(),
            input_nft.script.clone(),
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
        typed_output(buyer_lock, input_nft.script.clone(), 90_000_000_000),
        nft_payload.clone(),
    );
    let udt_payment_output = TestCellOutput::new(
        typed_output(payment_lock, payment_udt.script.clone(), 90_000_000_000),
        udt_amount_data(payment_amount),
    );
    let dummy_base_input = if matches!(
        case,
        LimitOrderLockFillCase::OrderInputInAppendScope
            | LimitOrderLockFillCase::PaymentInAnotherOtx
    ) {
        Some(live_input(
            fixture.context_mut(),
            normal_output(always_success.script.clone(), 100_000_000_000),
            Vec::new(),
        ))
    } else {
        None
    };
    let remainder_payment_output = if matches!(
        case,
        LimitOrderLockFillCase::TxLevelRemainderOnly
            | LimitOrderLockFillCase::PaymentOutputOutOfRange
    ) {
        Some(TestCellOutput::new(
            typed_output(owner_lock.clone(), udt.script.clone(), 90_000_000_000),
            udt_amount_data(if case == LimitOrderLockFillCase::PaymentOutputOutOfRange {
                30
            } else {
                1
            }),
        ))
    } else {
        None
    };
    let other_otx_payment_output = if case == LimitOrderLockFillCase::PaymentInAnotherOtx {
        Some(TestCellOutput::new(
            typed_output(owner_lock.clone(), udt.script.clone(), 90_000_000_000),
            udt_amount_data(1),
        ))
    } else {
        None
    };

    let action_target = if case == LimitOrderLockFillCase::WrongActionTarget {
        [8; 32]
    } else {
        order_lock_hash
    };
    let action_requested_asset = if case == LimitOrderLockFillCase::RequestedAssetMismatch {
        wrong_udt.script_hash
    } else {
        udt.script_hash
    };
    let action_requested_amount = if case == LimitOrderLockFillCase::MinRequestedBelowRequired {
        29
    } else {
        30
    };
    let payment_output_index = match case {
        LimitOrderLockFillCase::PaymentInAnotherOtx
        | LimitOrderLockFillCase::PaymentOutputOutOfRange => 2,
        _ => 1,
    };
    let fill_order_message = fixture
        .cobuild()
        .input_lock_action(action_target)
        .action_data(action_data(
            case,
            action_requested_asset,
            action_requested_amount,
            payment_output_index,
        ))
        .build();
    let otx_message = if case == LimitOrderLockFillCase::TxLevelFillOrder {
        empty_message()
    } else {
        fill_order_message.clone()
    };
    let append_input_cells = if case == LimitOrderLockFillCase::OrderInputInAppendScope {
        2
    } else {
        1
    };
    let seal_scope = if case == LimitOrderLockFillCase::OrderInputInAppendScope {
        1
    } else {
        0
    };
    let otx = fixture
        .otx()
        .base_input_cells(1)
        .base_output_cells(1)
        .append_input_cells(append_input_cells)
        .append_output_cells(1)
        .allow_append_inputs()
        .allow_append_outputs()
        .message(otx_message)
        .seals(vec![seal_pair(order_lock_hash, seal_scope, Vec::new())])
        .build_with_layout();
    let other_otx = if case == LimitOrderLockFillCase::PaymentInAnotherOtx {
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
        .cell_dep(cell_dep_for_script(&limit_order_lock_code))
        .cell_dep(cell_dep_for_script(&always_success))
        .cell_dep(cell_dep_for_script(&wrong_owner))
        .cell_dep(cell_dep_for_script(&nft))
        .cell_dep(cell_dep_for_script(&wrong_nft))
        .cell_dep(cell_dep_for_script(&udt))
        .cell_dep(cell_dep_for_script(&wrong_udt));
    if case == LimitOrderLockFillCase::OrderInputInAppendScope {
        tx = tx
            .base_input(dummy_base_input.expect("dummy base input"))
            .append_input(nft_input)
            .append_input(udt_input);
    } else {
        tx = tx.base_input(nft_input).append_input(udt_input);
        if let Some(input) = dummy_base_input {
            tx = tx.base_input(input);
        }
    }
    tx = tx.base_output(nft_output).append_output(udt_payment_output);
    if let Some(output) = remainder_payment_output {
        tx = tx.remainder_output(output);
    }
    if let Some(output) = other_otx_payment_output {
        tx = tx.append_output(output);
    }
    if case == LimitOrderLockFillCase::TxLevelFillOrder {
        tx = tx.tx_level_message(fill_order_message);
    }
    tx = tx.otx(otx);
    if let Some(other_otx) = other_otx {
        tx = tx.otx(other_otx);
    }
    let tx = tx.build();

    (fixture, tx)
}

fn limit_order_two_lock_orders_case(
    case: LimitOrderLockFillCase,
) -> (CobuildTestFixture, TransactionView) {
    let mut fixture = CobuildTestFixture::new();
    let limit_order_lock_code =
        deploy_data2_script(fixture.context_mut(), "limit-order-lock", Vec::new());
    let always_success = fixture.deploy_always_success();
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let nft_a = deploy_test_nft(&mut fixture, [0x71; 32]);
    let nft_b = deploy_test_nft(&mut fixture, [0x72; 32]);
    let udt = deploy_test_udt_with_owner(&mut fixture, issuer_lock_hash);

    let order_a = LockOrder {
        owner_lock_hash: script_hash(&owner_lock),
        offered_nft_type_hash: nft_a.script_hash,
        requested_asset_id: udt.script_hash,
        min_requested_amount: 30,
    };
    let order_b = LockOrder {
        offered_nft_type_hash: nft_b.script_hash,
        ..order_a
    };
    let order_lock_a = fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_lock_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&lock_args(order_a)),
        )
        .expect("build first limit order lock");
    let order_lock_b = fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_lock_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&lock_args(order_b)),
        )
        .expect("build second limit order lock");
    let order_lock_hash_a = script_hash(&order_lock_a);
    let order_lock_hash_b = script_hash(&order_lock_b);

    let nft_payload_a = nft_data(b"lock-order-a", [1, 2, 3, 4], 1_717_171_717);
    let nft_payload_b = nft_data(b"lock-order-b", [5, 6, 7, 8], 1_717_171_718);
    let nft_input_a = live_input(
        fixture.context_mut(),
        typed_output(order_lock_a, nft_a.script.clone(), 100_000_000_000),
        nft_payload_a.clone(),
    );
    let nft_input_b = live_input(
        fixture.context_mut(),
        typed_output(order_lock_b, nft_b.script.clone(), 100_000_000_000),
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
        typed_output(buyer_lock, nft_b.script.clone(), 90_000_000_000),
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
    let second_payment_index = if case == LimitOrderLockFillCase::TwoLockOrdersReusePaymentOutput {
        2
    } else {
        3
    };
    let message = fixture
        .cobuild()
        .push_action(
            0,
            order_lock_hash_a,
            fill_action_data(udt.script_hash, 30, 2),
        )
        .push_action(
            0,
            order_lock_hash_b,
            fill_action_data(udt.script_hash, 30, second_payment_index),
        )
        .build();
    let otx = fixture
        .otx()
        .base_input_cells(2)
        .base_output_cells(2)
        .append_input_cells(1)
        .append_output_cells(2)
        .allow_append_inputs()
        .allow_append_outputs()
        .message(message)
        .seals(vec![
            seal_pair(order_lock_hash_a, 0, Vec::new()),
            seal_pair(order_lock_hash_b, 0, Vec::new()),
        ])
        .build_with_layout();

    let tx = fixture
        .tx()
        .cell_dep(cell_dep_for_script(&limit_order_lock_code))
        .cell_dep(cell_dep_for_script(&always_success))
        .cell_dep(cell_dep_for_script(&nft_a))
        .cell_dep(cell_dep_for_script(&nft_b))
        .cell_dep(cell_dep_for_script(&udt))
        .base_input(nft_input_a)
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

fn action_data(
    case: LimitOrderLockFillCase,
    requested_asset_id: [u8; 32],
    amount: u64,
    payment_output_index: u32,
) -> Vec<u8> {
    match case {
        LimitOrderLockFillCase::UnknownActionTag => {
            let mut data = Vec::with_capacity(45);
            data.push(1);
            data.extend_from_slice(&[0u8; 44]);
            data
        }
        LimitOrderLockFillCase::MalformedAction => {
            let mut data = fill_action_data(requested_asset_id, amount, payment_output_index);
            data.pop();
            data
        }
        _ => fill_action_data(requested_asset_id, amount, payment_output_index),
    }
}

fn fill_action_data(
    requested_asset_id: [u8; 32],
    amount: u64,
    payment_output_index: u32,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(45);
    data.push(FILL_ORDER_TAG);
    data.extend_from_slice(&requested_asset_id);
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&payment_output_index.to_le_bytes());
    data
}

fn lock_args(order: LockOrder) -> Vec<u8> {
    let mut data = Vec::with_capacity(104);
    data.extend_from_slice(&order.owner_lock_hash);
    data.extend_from_slice(&order.offered_nft_type_hash);
    data.extend_from_slice(&order.requested_asset_id);
    data.extend_from_slice(&order.min_requested_amount.to_le_bytes());
    data
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

fn deploy_wrong_owner_lock(fixture: &mut CobuildTestFixture) -> DeployedScript {
    deploy_always_success(fixture.context_mut(), b"wrong-owner".to_vec())
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
