use ckb_testtool::{
    builtin::ALWAYS_SUCCESS,
    ckb_error::ErrorKind,
    ckb_script::{ScriptError, TransactionScriptError},
    ckb_types::{
        bytes::Bytes,
        core::{ScriptHashType, TransactionBuilder, TransactionView},
        packed::{CellDep, CellInput, CellOutput},
        prelude::*,
    },
    context::Context,
};
use cobuild_types::entity::{
    core::{Action, ActionVec, Message as CobuildMessage, Otx, OtxStart, SealPairVec},
    witness::WitnessLayout,
};
use tests::{Loader, verify_and_dump_failed_tx};

const ORDER_ID: [u8; 32] = [1; 32];
const OFFERED_ASSET_ID: [u8; 32] = [3; 32];
const REQUESTED_ASSET_ID: [u8; 32] = [4; 32];
const FILL_ORDER_TAG: u8 = 1;

#[test]
fn limit_order_accepts_otx_append_settlement_at_limit_price() {
    let (context, tx) = limit_order_case(30);

    let result = verify_and_dump_failed_tx(&context, &tx, 50_000_000);

    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn limit_order_rejects_otx_append_settlement_below_limit_price() {
    let (context, tx) = limit_order_case(29);

    let result = verify_and_dump_failed_tx(&context, &tx, 50_000_000);

    assert_type_script_exit(result, 11);
}

fn limit_order_case(settlement_amount: u64) -> (Context, TransactionView) {
    let mut context = Context::default();

    let limit_order_bin = Loader::default().load_binary("limit-order");
    let limit_order_out_point = context.deploy_cell(limit_order_bin);
    let limit_order_dep = CellDep::new_builder()
        .out_point(limit_order_out_point.clone())
        .build();
    let limit_order_type = context
        .build_script_with_hash_type(&limit_order_out_point, ScriptHashType::Data2, Bytes::new())
        .expect("build limit-order type script");
    let limit_order_type_hash = packed_hash_to_array(limit_order_type.calc_script_hash());

    let always_out_point = context.deploy_cell(ALWAYS_SUCCESS.to_vec().into());
    let always_dep = CellDep::new_builder()
        .out_point(always_out_point.clone())
        .build();
    let owner_lock = context
        .build_script_with_hash_type(&always_out_point, ScriptHashType::Data, Bytes::new())
        .expect("build owner lock");
    let owner_lock_hash = packed_hash_to_array(owner_lock.calc_script_hash());

    let order_input_output = CellOutput::new_builder()
        .capacity(100_000_000_000u64)
        .lock(owner_lock.clone())
        .type_(Some(limit_order_type).pack())
        .build();
    let order_input_data = order_data(owner_lock_hash, 10, 3);
    let order_input_out_point = context.create_cell(order_input_output, order_input_data.into());

    let settlement_output = CellOutput::new_builder()
        .capacity(90_000_000_000u64)
        .lock(owner_lock)
        .build();
    let settlement_data = settlement_data(settlement_amount);

    let otx_start = WitnessLayout::from(
        OtxStart::new_builder()
            .start_input_cell(0u32.to_le_bytes())
            .start_output_cell(0u32.to_le_bytes())
            .start_cell_deps(0u32.to_le_bytes())
            .start_header_deps(0u32.to_le_bytes())
            .build(),
    );
    let otx = WitnessLayout::from(otx_witness(limit_order_type_hash));

    TransactionBuilder::default()
        .cell_dep(limit_order_dep)
        .cell_dep(always_dep)
        .input(
            CellInput::new_builder()
                .previous_output(order_input_out_point)
                .build(),
        )
        .output(settlement_output)
        .output_data(settlement_data)
        .witness(Bytes::copy_from_slice(otx_start.as_slice()).pack())
        .witness(Bytes::copy_from_slice(otx.as_slice()).pack())
        .build()
        .pipe(|tx| (context, tx))
}

fn otx_witness(limit_order_type_hash: [u8; 32]) -> Otx {
    Otx::new_builder()
        .message(fill_message(limit_order_type_hash))
        .append_permissions(0b0010u8)
        .base_input_cells(1u32.to_le_bytes())
        .base_input_masks(vec![0u8])
        .base_output_cells(0u32.to_le_bytes())
        .base_output_masks(Vec::<u8>::new())
        .base_cell_deps(0u32.to_le_bytes())
        .base_cell_dep_masks(Vec::<u8>::new())
        .base_header_deps(0u32.to_le_bytes())
        .base_header_dep_masks(Vec::<u8>::new())
        .append_input_cells(0u32.to_le_bytes())
        .append_output_cells(1u32.to_le_bytes())
        .append_cell_deps(0u32.to_le_bytes())
        .append_header_deps(0u32.to_le_bytes())
        .seals(SealPairVec::new_builder().build())
        .build()
}

fn fill_message(limit_order_type_hash: [u8; 32]) -> CobuildMessage {
    let action = Action::new_builder()
        .script_info_hash([0u8; 32])
        .script_role(1u8)
        .script_hash(limit_order_type_hash)
        .data(fill_action_data())
        .build();

    CobuildMessage::new_builder()
        .actions(ActionVec::new_builder().push(action).build())
        .build()
}

fn fill_action_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.push(FILL_ORDER_TAG);
    data.extend_from_slice(&ORDER_ID);
    data.extend_from_slice(&REQUESTED_ASSET_ID);
    data.extend_from_slice(&10u64.to_le_bytes());
    data.extend_from_slice(&30u64.to_le_bytes());
    data
}

fn order_data(owner_lock_hash: [u8; 32], offered_remaining: u64, min_price: u64) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&ORDER_ID);
    data.extend_from_slice(&owner_lock_hash);
    data.extend_from_slice(&OFFERED_ASSET_ID);
    data.extend_from_slice(&REQUESTED_ASSET_ID);
    data.extend_from_slice(&offered_remaining.to_le_bytes());
    data.extend_from_slice(&min_price.to_le_bytes());
    data.extend_from_slice(&9u64.to_le_bytes());
    data
}

fn settlement_data(amount: u64) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&REQUESTED_ASSET_ID);
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

fn packed_hash_to_array(hash: ckb_testtool::ckb_types::packed::Byte32) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(hash.as_slice());
    out
}

fn assert_type_script_exit(result: Result<u64, ckb_testtool::ckb_error::Error>, code: i8) {
    let err = result.expect_err("transaction must fail closed");
    assert_eq!(err.kind(), ErrorKind::Script);

    let script_error = err
        .root_cause()
        .downcast_ref::<TransactionScriptError>()
        .expect("script validation error");
    assert_eq!(
        script_error.originating_script().to_string(),
        "Inputs[0].Type"
    );
    assert!(
        matches!(
            script_error.script_error(),
            ScriptError::ValidationFailure(_, actual) if *actual == code
        ),
        "{script_error:?}"
    );
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}
