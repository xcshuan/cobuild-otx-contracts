use blake2b_ref::Blake2b;
use ckb_testtool::ckb_types::{core::TransactionView, prelude::Entity};
use cobuild_core::{
    error::CoreError,
    layout::{OtxLayout, Range},
    reader::{cursor_from_slice, update_cursor_with_error},
    view::{CobuildWitnessLayoutView, OtxView},
};
use cobuild_types::lazy_reader::blockchain::{CellInput, CellOutput};

use crate::framework::tx::{BuiltTxShape, OtxHandle, OtxRangeFacts};

use super::tx::{checked_len_prefix, new_hasher};

const OTX_BASE_PERSONAL: &[u8; 16] = b"ckbcb_otb_core1\0";
const OTX_APPEND_PERSONAL: &[u8; 16] = b"ckbcb_ota_core1\0";

pub(crate) fn otx_base_hash(built: &BuiltTxShape, otx: OtxHandle) -> [u8; 32] {
    let (view, layout) = otx_hash_inputs(built, otx);
    let mut out = [0u8; 32];
    let mut hasher = new_hasher(OTX_BASE_PERSONAL);

    update_cursor_with_error(&mut hasher, &view.message, CoreError::MalformedCobuild)
        .expect("message cursor");
    hasher.update(&[view.append_permissions]);
    write_otx_base_input_cells(&mut hasher, built, &view, &layout);
    write_otx_base_output_cells(&mut hasher, built, &view, &layout);
    write_otx_base_cell_deps(&mut hasher, built, &view, &layout);
    write_otx_base_header_deps(&mut hasher, built, &view, &layout);

    hasher.finalize(&mut out);
    out
}

pub(crate) fn otx_append_hash(
    built: &BuiltTxShape,
    otx: OtxHandle,
    base_hash: [u8; 32],
) -> [u8; 32] {
    let (view, layout) = otx_hash_inputs(built, otx);
    let mut out = [0u8; 32];
    let mut hasher = new_hasher(OTX_APPEND_PERSONAL);

    update_cursor_with_error(&mut hasher, &view.message, CoreError::MalformedCobuild)
        .expect("message cursor");
    hasher.update(&base_hash);
    write_otx_append_input_cells(&mut hasher, built, &view, &layout);
    write_otx_append_output_cells(&mut hasher, built, &view, &layout);
    write_otx_append_cell_deps(&mut hasher, built, &view, &layout);
    write_otx_append_header_deps(&mut hasher, built, &view, &layout);

    hasher.finalize(&mut out);
    out
}

fn write_otx_base_input_cells(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    otx: &OtxView,
    layout: &OtxLayout,
) {
    write_count(hasher, otx.base_input_cells);
    write_len_prefixed_bytes(hasher, otx.base_input_masks.bytes());
    for local_index in 0..otx.base_input_cells {
        let tx_index = checked_index(layout.base_inputs, local_index);
        let input_bytes = built.resolved_inputs[tx_index].input.as_slice();
        let input = CellInput::from(cursor_from_slice(input_bytes));

        write_count(hasher, local_index);
        if otx
            .includes_base_input_since(local_index)
            .expect("input mask")
        {
            hasher.update(&input.since().expect("since").to_le_bytes());
        }
        if otx
            .includes_base_input_previous_output(local_index)
            .expect("input mask")
        {
            update_cursor_with_error(
                hasher,
                &input.previous_output().expect("previous output").cursor,
                CoreError::MissingHashInput,
            )
            .expect("previous output cursor");
        }
        hasher.update(built.resolved_inputs[tx_index].output.as_slice());
        write_len_prefixed_bytes(hasher, built.resolved_inputs[tx_index].data.as_ref());
    }
}

fn write_otx_base_output_cells(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    otx: &OtxView,
    layout: &OtxLayout,
) {
    write_count(hasher, otx.base_output_cells);
    write_len_prefixed_bytes(hasher, otx.base_output_masks.bytes());
    for local_index in 0..otx.base_output_cells {
        let tx_index = checked_index(layout.base_outputs, local_index);
        let output_bytes = raw_output_bytes(&built.tx, tx_index);
        let output = cursor_from_slice(&output_bytes);
        let output_view = CellOutput::from(output.clone());

        write_count(hasher, local_index);
        if otx
            .includes_base_output_capacity(local_index)
            .expect("output mask")
        {
            hasher.update(&output_view.capacity().expect("capacity").to_le_bytes());
        }
        if otx
            .includes_base_output_lock(local_index)
            .expect("output mask")
        {
            update_cursor_with_error(
                hasher,
                &output_view.lock().expect("lock").cursor,
                CoreError::MissingHashInput,
            )
            .expect("lock cursor");
        }
        if otx
            .includes_base_output_type(local_index)
            .expect("output mask")
        {
            update_cursor_with_error(
                hasher,
                &output
                    .table_slice_by_index(2)
                    .expect("output type option cursor"),
                CoreError::MissingHashInput,
            )
            .expect("type cursor");
        }
        if otx
            .includes_base_output_data(local_index)
            .expect("output mask")
        {
            write_len_prefixed_bytes(hasher, &raw_output_data_bytes(&built.tx, tx_index));
        }
    }
}

fn write_otx_base_cell_deps(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    otx: &OtxView,
    layout: &OtxLayout,
) {
    write_count(hasher, otx.base_cell_deps);
    write_len_prefixed_bytes(hasher, otx.base_cell_dep_masks.bytes());
    for local_index in 0..otx.base_cell_deps {
        if otx
            .base_cell_dep_masks
            .get(local_index)
            .expect("cell dep mask")
        {
            let tx_index = checked_index(layout.base_cell_deps, local_index);
            write_count(hasher, local_index);
            hasher.update(&raw_cell_dep_bytes(&built.tx, tx_index));
        }
    }
}

fn write_otx_base_header_deps(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    otx: &OtxView,
    layout: &OtxLayout,
) {
    write_count(hasher, otx.base_header_deps);
    write_len_prefixed_bytes(hasher, otx.base_header_dep_masks.bytes());
    for local_index in 0..otx.base_header_deps {
        if otx
            .base_header_dep_masks
            .get(local_index)
            .expect("header dep mask")
        {
            let tx_index = checked_index(layout.base_header_deps, local_index);
            write_count(hasher, local_index);
            hasher.update(&raw_header_dep_hash(&built.tx, tx_index));
        }
    }
}

fn write_otx_append_input_cells(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    otx: &OtxView,
    layout: &OtxLayout,
) {
    write_count(hasher, otx.append_input_cells);
    for local_index in 0..otx.append_input_cells {
        let tx_index = checked_index(layout.append_inputs, local_index);
        write_count(hasher, local_index);
        hasher.update(built.resolved_inputs[tx_index].input.as_slice());
        hasher.update(built.resolved_inputs[tx_index].output.as_slice());
        write_len_prefixed_bytes(hasher, built.resolved_inputs[tx_index].data.as_ref());
    }
}

fn write_otx_append_output_cells(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    otx: &OtxView,
    layout: &OtxLayout,
) {
    write_count(hasher, otx.append_output_cells);
    for local_index in 0..otx.append_output_cells {
        let tx_index = checked_index(layout.append_outputs, local_index);
        write_count(hasher, local_index);
        hasher.update(&raw_output_bytes(&built.tx, tx_index));
        write_len_prefixed_bytes(hasher, &raw_output_data_bytes(&built.tx, tx_index));
    }
}

fn write_otx_append_cell_deps(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    otx: &OtxView,
    layout: &OtxLayout,
) {
    write_count(hasher, otx.append_cell_deps);
    for local_index in 0..otx.append_cell_deps {
        let tx_index = checked_index(layout.append_cell_deps, local_index);
        write_count(hasher, local_index);
        hasher.update(&raw_cell_dep_bytes(&built.tx, tx_index));
    }
}

fn write_otx_append_header_deps(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    otx: &OtxView,
    layout: &OtxLayout,
) {
    write_count(hasher, otx.append_header_deps);
    for local_index in 0..otx.append_header_deps {
        let tx_index = checked_index(layout.append_header_deps, local_index);
        write_count(hasher, local_index);
        hasher.update(&raw_header_dep_hash(&built.tx, tx_index));
    }
}

fn otx_hash_inputs(built: &BuiltTxShape, otx: OtxHandle) -> (OtxView, OtxLayout) {
    let witness = built
        .tx
        .witnesses()
        .into_iter()
        .nth(otx.0 + 1)
        .expect("OTX witness");
    let view =
        CobuildWitnessLayoutView::from_cursor(cursor_from_slice(witness.raw_data().as_ref()))
            .expect("cobuild witness layout")
            .otx()
            .expect("parse OTX")
            .expect("OTX witness layout");
    let facts = otx_range_facts(built, otx);
    let layout = OtxLayout {
        witness_index: otx.0 + 1,
        base_inputs: range(&facts.base_inputs),
        append_inputs: range(&facts.append_inputs),
        base_outputs: range(&facts.base_outputs),
        append_outputs: range(&facts.append_outputs),
        base_cell_deps: range(&facts.base_cell_deps),
        append_cell_deps: range(&facts.append_cell_deps),
        base_header_deps: range(&facts.base_header_deps),
        append_header_deps: range(&facts.append_header_deps),
    };
    (view, layout)
}

fn otx_range_facts(built: &BuiltTxShape, otx: OtxHandle) -> &OtxRangeFacts {
    built
        .otx_ranges
        .iter()
        .find(|facts| facts.otx == otx)
        .expect("unknown OTX handle")
}

fn range(range: &std::ops::Range<usize>) -> Range {
    Range {
        start: range.start,
        count: range.end.saturating_sub(range.start),
    }
}

fn checked_index(range: Range, local_index: usize) -> usize {
    assert!(local_index < range.count, "OTX local index out of range");
    range.start + local_index
}

fn raw_output_bytes(tx: &TransactionView, index: usize) -> Vec<u8> {
    tx.outputs()
        .into_iter()
        .nth(index)
        .expect("transaction output")
        .as_slice()
        .to_vec()
}

fn raw_output_data_bytes(tx: &TransactionView, index: usize) -> Vec<u8> {
    tx.outputs_data()
        .into_iter()
        .nth(index)
        .expect("transaction output data")
        .raw_data()
        .to_vec()
}

fn raw_cell_dep_bytes(tx: &TransactionView, index: usize) -> Vec<u8> {
    tx.cell_deps()
        .into_iter()
        .nth(index)
        .expect("transaction cell dep")
        .as_slice()
        .to_vec()
}

fn raw_header_dep_hash(tx: &TransactionView, index: usize) -> [u8; 32] {
    let dep = tx
        .header_deps()
        .into_iter()
        .nth(index)
        .expect("transaction header dep");
    let mut out = [0u8; 32];
    out.copy_from_slice(dep.as_slice());
    out
}

fn write_count(hasher: &mut Blake2b, count: usize) {
    hasher.update(&checked_len_prefix(count));
}

fn write_len_prefixed_bytes(hasher: &mut Blake2b, bytes: &[u8]) {
    hasher.update(&checked_len_prefix(bytes.len()));
    hasher.update(bytes);
}
