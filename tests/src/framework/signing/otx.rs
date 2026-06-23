use blake2b_ref::Blake2b;
use ckb_testtool::ckb_types::{
    core::TransactionView,
    packed,
    prelude::{Builder, Entity},
};
use cobuild_core::{
    error::CoreError,
    layout::{OtxAppendSegmentLayout, OtxLayout, Range},
    protocol::SegmentFlags,
    reader::{cursor_from_slice, update_cursor_with_error},
    view::{CobuildWitnessLayoutView, OtxView},
};
use cobuild_types::lazy_reader::blockchain::{CellInput, CellOutput};

use crate::framework::tx::{BuiltTxShape, OtxHandle, OtxRangeFacts};

use super::tx::{checked_len_prefix, new_hasher};

const OTX_BASE_PERSONAL: &[u8; 16] = b"ckbcb_otb_core1\0";
const OTX_APPEND_SEGMENT_PERSONAL: &[u8; 16] = b"ckbcb_ots_core1\0";

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

pub(crate) fn otx_append_segment_hash(
    built: &BuiltTxShape,
    otx: OtxHandle,
    selected_segment_index: usize,
    base_hash: [u8; 32],
) -> [u8; 32] {
    let (view, layout) = otx_hash_inputs(built, otx);
    let segment = layout
        .append_segments
        .get(selected_segment_index)
        .expect("append segment layout");
    view.append_segments
        .get(selected_segment_index)
        .expect("append segment view");
    let mut out = [0u8; 32];
    let mut hasher = new_hasher(OTX_APPEND_SEGMENT_PERSONAL);

    hasher.update(&base_hash);
    hasher.update(&[segment.flags.raw()]);
    if segment.flags.coverage_previous_segments() {
        // Own-only segments use selected_segment_index only as a lookup key. Previous-coverage
        // segments bind their position through the number of previous ordered segments.
        let previous_segment_count = selected_segment_index;
        write_count(&mut hasher, previous_segment_count);
        for previous_segment_index in 0..previous_segment_count {
            let previous_segment = layout
                .append_segments
                .get(previous_segment_index)
                .expect("previous append segment layout");
            write_count(&mut hasher, previous_segment_index);
            hasher.update(&[previous_segment.flags.raw()]);
            write_otx_append_segment_entities(
                &mut hasher,
                built,
                &view,
                &layout,
                previous_segment_index,
            );
        }
    }
    write_otx_append_segment_entities(&mut hasher, built, &view, &layout, selected_segment_index);

    hasher.finalize(&mut out);
    out
}

fn write_default_out_point(hasher: &mut Blake2b) {
    let value = packed::OutPoint::new_builder().build();
    hasher.update(value.as_slice());
}

fn write_default_script(hasher: &mut Blake2b) {
    let value = packed::Script::new_builder().build();
    hasher.update(value.as_slice());
}

fn write_default_script_opt(hasher: &mut Blake2b) {
    let value = packed::ScriptOpt::new_builder().build();
    hasher.update(value.as_slice());
}

fn write_default_cell_dep(hasher: &mut Blake2b) {
    let value = packed::CellDep::new_builder().build();
    hasher.update(value.as_slice());
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
        } else {
            hasher.update(&0u64.to_le_bytes());
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
        } else {
            write_default_out_point(hasher);
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
        } else {
            hasher.update(&0u64.to_le_bytes());
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
        } else {
            write_default_script(hasher);
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
        } else {
            write_default_script_opt(hasher);
        }
        if otx
            .includes_base_output_data(local_index)
            .expect("output mask")
        {
            write_len_prefixed_bytes(hasher, &raw_output_data_bytes(&built.tx, tx_index));
        } else {
            write_len_prefixed_bytes(hasher, &[]);
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
        write_count(hasher, local_index);
        if otx
            .base_cell_dep_masks
            .get(local_index)
            .expect("cell dep mask")
        {
            let tx_index = checked_index(layout.base_cell_deps, local_index);
            hasher.update(&raw_cell_dep_bytes(&built.tx, tx_index));
        } else {
            write_default_cell_dep(hasher);
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
        write_count(hasher, local_index);
        if otx
            .base_header_dep_masks
            .get(local_index)
            .expect("header dep mask")
        {
            let tx_index = checked_index(layout.base_header_deps, local_index);
            hasher.update(&raw_header_dep_hash(&built.tx, tx_index));
        } else {
            hasher.update(&[0u8; 32]);
        }
    }
}

fn write_otx_append_segment_entities(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    otx: &OtxView,
    layout: &OtxLayout,
    segment_index: usize,
) {
    let segment = otx
        .append_segments
        .get(segment_index)
        .expect("append segment view");
    let segment_layout = layout
        .append_segments
        .get(segment_index)
        .expect("append segment layout");
    write_otx_append_input_cells(hasher, built, segment.input_cells, segment_layout.inputs);
    write_otx_append_output_cells(hasher, built, segment.output_cells, segment_layout.outputs);
    write_otx_append_cell_deps(hasher, built, segment.cell_deps, segment_layout.cell_deps);
    write_otx_append_header_deps(
        hasher,
        built,
        segment.header_deps,
        segment_layout.header_deps,
    );
}

fn write_otx_append_input_cells(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    count: usize,
    range: Range,
) {
    write_count(hasher, count);
    for local_index in 0..count {
        let tx_index = checked_index(range, local_index);
        write_count(hasher, local_index);
        hasher.update(built.resolved_inputs[tx_index].input.as_slice());
        hasher.update(built.resolved_inputs[tx_index].output.as_slice());
        write_len_prefixed_bytes(hasher, built.resolved_inputs[tx_index].data.as_ref());
    }
}

fn write_otx_append_output_cells(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    count: usize,
    range: Range,
) {
    write_count(hasher, count);
    for local_index in 0..count {
        let tx_index = checked_index(range, local_index);
        write_count(hasher, local_index);
        hasher.update(&raw_output_bytes(&built.tx, tx_index));
        write_len_prefixed_bytes(hasher, &raw_output_data_bytes(&built.tx, tx_index));
    }
}

fn write_otx_append_cell_deps(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    count: usize,
    range: Range,
) {
    write_count(hasher, count);
    for local_index in 0..count {
        let tx_index = checked_index(range, local_index);
        write_count(hasher, local_index);
        hasher.update(&raw_cell_dep_bytes(&built.tx, tx_index));
    }
}

fn write_otx_append_header_deps(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    count: usize,
    range: Range,
) {
    write_count(hasher, count);
    for local_index in 0..count {
        let tx_index = checked_index(range, local_index);
        write_count(hasher, local_index);
        hasher.update(&raw_header_dep_hash(&built.tx, tx_index));
    }
}

fn otx_hash_inputs(built: &BuiltTxShape, otx: OtxHandle) -> (OtxView, OtxLayout) {
    let witness_index = built.witnesses.tx_index(built.otx_witness(otx));
    let witness = built
        .tx
        .witnesses()
        .into_iter()
        .nth(witness_index)
        .expect("OTX witness");
    let view =
        CobuildWitnessLayoutView::from_cursor(cursor_from_slice(witness.raw_data().as_ref()))
            .expect("cobuild witness layout")
            .otx()
            .expect("parse OTX")
            .expect("OTX witness layout");
    let facts = otx_range_facts(built, otx);
    let base_inputs = range_from_start_and_count(facts.base_inputs.start, view.base_input_cells);
    let base_outputs = range_from_start_and_count(facts.base_outputs.start, view.base_output_cells);
    let base_cell_deps =
        range_from_start_and_count(facts.base_cell_deps.start, view.base_cell_deps);
    let base_header_deps =
        range_from_start_and_count(facts.base_header_deps.start, view.base_header_deps);
    let mut next_input = base_inputs.end();
    let mut next_output = base_outputs.end();
    let mut next_cell_dep = base_cell_deps.end();
    let mut next_header_dep = base_header_deps.end();
    let append_segments = view
        .append_segments
        .iter()
        .map(|segment| OtxAppendSegmentLayout {
            flags: SegmentFlags::try_from(segment.segment_flags).expect("append segment flags"),
            inputs: take_range(&mut next_input, segment.input_cells),
            outputs: take_range(&mut next_output, segment.output_cells),
            cell_deps: take_range(&mut next_cell_dep, segment.cell_deps),
            header_deps: take_range(&mut next_header_dep, segment.header_deps),
        })
        .collect();
    let layout = OtxLayout {
        witness_index,
        base_inputs,
        append_inputs: Range {
            start: base_inputs.end(),
            count: next_input - base_inputs.end(),
        },
        base_outputs,
        append_outputs: Range {
            start: base_outputs.end(),
            count: next_output - base_outputs.end(),
        },
        base_cell_deps,
        append_cell_deps: Range {
            start: base_cell_deps.end(),
            count: next_cell_dep - base_cell_deps.end(),
        },
        base_header_deps,
        append_header_deps: Range {
            start: base_header_deps.end(),
            count: next_header_dep - base_header_deps.end(),
        },
        append_segments,
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

fn range_from_start_and_count(start: usize, count: usize) -> Range {
    Range { start, count }
}

fn take_range(next: &mut usize, count: usize) -> Range {
    let range = Range {
        start: *next,
        count,
    };
    *next = next.checked_add(count).expect("valid OTX layout range");
    range
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
