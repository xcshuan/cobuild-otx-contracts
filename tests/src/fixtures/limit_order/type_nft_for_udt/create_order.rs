use super::*;

use ckb_hash::new_blake2b;
use ckb_testtool::ckb_types::packed::CellInput;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CreateOrderCase {
    Valid,
    MissingNftProxyOutput,
    WrongNftType,
    WrongProxyOrder,
    StateActionMismatch,
    InvalidTypeId,
    InputAndOutputGroupShape,
}

pub fn type_script_create_order_cases() -> Vec<BuiltLimitOrderCase> {
    vec![
        create_order_case(CreateOrderCase::Valid),
        create_order_case(CreateOrderCase::MissingNftProxyOutput),
        create_order_case(CreateOrderCase::WrongNftType),
        create_order_case(CreateOrderCase::WrongProxyOrder),
        create_order_case(CreateOrderCase::StateActionMismatch),
        create_order_case(CreateOrderCase::InvalidTypeId),
        create_order_case(CreateOrderCase::InputAndOutputGroupShape),
    ]
}

fn create_order_case(case: CreateOrderCase) -> BuiltLimitOrderCase {
    let mut fixture = CobuildTestFixture::new();
    let limit_order_code = fixture.deploy_limit_order();
    let always_success = deploy_always_success(fixture.context_mut(), Vec::new());
    let owner_lock = always_success.script.clone();
    let funding_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(owner_lock.clone(), 200_000_000_000),
        Vec::new(),
    );
    let nft_type_id = type_id_args(&funding_input.input, 1);
    let nft = deploy_test_nft(fixture.context_mut(), nft_type_id);
    let output_nft = if case == CreateOrderCase::WrongNftType {
        deploy_test_nft(fixture.context_mut(), type_id_args(&funding_input.input, 2))
    } else {
        nft.clone()
    };
    let udt = deploy_test_udt(fixture.context_mut(), script_hash(&always_success.script));
    let computed_order_type_id = type_id_args(&funding_input.input, 0);
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
        Some(live_resolved_facts(
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

    let mut shape = TxShape::new();
    push_deps(
        &mut shape,
        [
            &limit_order_code,
            &always_success,
            &proxy_lock,
            &nft,
            &output_nft,
            &udt,
        ],
    );
    let order_input_handle = order_input.map(|input| shape.push_prefix_input(input));
    shape.push_prefix_input(funding_input);
    let order_output_handle = shape.push_remainder_output(order_output);
    if let Some(output) = wrong_nft_padding_output {
        shape.push_remainder_output(output);
    }
    if !matches!(
        case,
        CreateOrderCase::MissingNftProxyOutput | CreateOrderCase::InputAndOutputGroupShape
    ) {
        shape.push_remainder_output(nft_output);
    }
    shape.tx_level_message(empty_message());
    let mut built = shape.build();
    let action = LimitOrderAction::Create {
        order: action_state,
    };
    let message = fixture
        .cobuild()
        .output_type_action(order_type_hash)
        .action_data(encode_action(&action, &built))
        .build();
    replace_tx_level_message(&mut built, message);

    let expected = match case {
        CreateOrderCase::Valid => LimitOrderExpectedOutcome::Pass,
        CreateOrderCase::StateActionMismatch => output_type_error(
            order_output_handle,
            LimitOrderTypeError::StateActionMismatch,
        ),
        CreateOrderCase::InvalidTypeId => {
            output_type_error(order_output_handle, LimitOrderTypeError::InvalidTypeId)
        }
        CreateOrderCase::InputAndOutputGroupShape => input_type_error(
            order_input_handle.expect("order input handle"),
            LimitOrderTypeError::InputAndOutputGroupShape,
        ),
        _ => output_type_error(order_output_handle, LimitOrderTypeError::InvalidAction),
    };

    built_case(
        format!("create::{case:?}"),
        fixture,
        built,
        expected,
        create_coverage(case),
    )
}

fn create_coverage(case: CreateOrderCase) -> CoverageTag {
    let mutation = match case {
        CreateOrderCase::Valid => None,
        CreateOrderCase::MissingNftProxyOutput => {
            Some(BusinessMutation::CreateMissingNftProxyOutput)
        }
        CreateOrderCase::WrongNftType => Some(BusinessMutation::CreateWrongNftType),
        CreateOrderCase::WrongProxyOrder => Some(BusinessMutation::CreateWrongProxyOrder),
        CreateOrderCase::StateActionMismatch => Some(BusinessMutation::CreateStateActionMismatch),
        CreateOrderCase::InvalidTypeId => Some(BusinessMutation::CreateInvalidTypeId),
        CreateOrderCase::InputAndOutputGroupShape => {
            Some(BusinessMutation::CreateInputAndOutputGroupShape)
        }
    };
    coverage(
        FlowKind::TxLevel,
        ScriptRoleKind::OutputType,
        OtxScopeKind::Remainder,
        super::ActionSourceKind::TxLevel,
        mutation,
    )
}

fn push_deps<'a>(
    shape: &mut TxShape,
    scripts: impl IntoIterator<Item = &'a crate::framework::contracts::DeployedScript>,
) {
    for script in scripts {
        shape.push_prefix_cell_dep(cell_dep_for_script(script));
    }
}

fn type_id_args(first_input: &CellInput, output_index: u64) -> [u8; 32] {
    let mut blake2b = new_blake2b();
    blake2b.update(first_input.as_slice());
    blake2b.update(&output_index.to_le_bytes());
    let mut out = [0u8; 32];
    blake2b.finalize(&mut out);
    out
}
