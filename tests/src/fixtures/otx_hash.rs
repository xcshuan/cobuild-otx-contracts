// This module is intentionally fixture-local: it mirrors the cobuild-otx-lock
// preimage hashing contract for lock verification tests. General signing and
// witness helpers belong in tests::framework instead.

use super::support::{OtxFixtureParts, OtxHashFixture};
use crate::framework::signing::checked_len_prefix;
use blake2b_ref::Blake2bBuilder;
use cobuild_core::{
    layout::{OtxLayout, Range},
    reader::{cursor_from_slice, update_cursor_with_error},
    view::{MaskView, OtxView},
};

const OTX_BASE_PERSONAL: &[u8; 16] = b"ckbcb_otb_core1\0";
const OTX_APPEND_PERSONAL: &[u8; 16] = b"ckbcb_ota_core1\0";

fn write_count(hasher: &mut blake2b_ref::Blake2b, count: usize) {
    hasher.update(&checked_len_prefix(count));
}

fn write_len_prefixed_bytes(hasher: &mut blake2b_ref::Blake2b, bytes: &[u8]) {
    hasher.update(&checked_len_prefix(bytes.len()));
    hasher.update(bytes);
}

pub(super) fn otx_base_hash(parts: &OtxFixtureParts) -> [u8; 32] {
    otx_base_hash_with_base_output_start(parts, 0)
}

pub(super) fn otx_base_hash_with_base_output_start(
    parts: &OtxFixtureParts,
    base_output_start: usize,
) -> [u8; 32] {
    let (otx, layout, fixture) = otx_hash_inputs(parts, base_output_start);
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32).personal(OTX_BASE_PERSONAL).build();

    update_cursor_with_error(
        &mut hasher,
        &otx.message,
        cobuild_core::error::CoreError::MalformedCobuild,
    )
    .expect("message cursor");
    hasher.update(&[otx.append_permissions]);
    write_count(&mut hasher, otx.base_input_cells);
    write_len_prefixed_bytes(&mut hasher, otx.base_input_masks.bytes());
    for local_index in 0..otx.base_input_cells {
        let tx_index = layout.base_inputs.start + local_index;
        let input = cursor_from_slice(&fixture.raw_inputs[tx_index]);
        let input_view = cobuild_types::lazy_reader::blockchain::CellInput::from(input.clone());

        write_count(&mut hasher, local_index);
        if otx
            .includes_base_input_since(local_index)
            .expect("input mask")
        {
            hasher.update(&input_view.since().expect("since").to_le_bytes());
        }
        if otx
            .includes_base_input_previous_output(local_index)
            .expect("input mask")
        {
            update_cursor_with_error(
                &mut hasher,
                &input_view
                    .previous_output()
                    .expect("previous output")
                    .cursor,
                cobuild_core::error::CoreError::MissingHashInput,
            )
            .expect("previous output cursor");
        }
        hasher.update(&fixture.resolved_outputs[tx_index]);
        hasher.update(&checked_len_prefix(fixture.resolved_data[tx_index].len()));
        hasher.update(&fixture.resolved_data[tx_index]);
    }

    write_count(&mut hasher, otx.base_output_cells);
    write_len_prefixed_bytes(&mut hasher, otx.base_output_masks.bytes());
    for local_index in 0..otx.base_output_cells {
        let tx_index = layout.base_outputs.start + local_index;
        let output = cursor_from_slice(&fixture.raw_outputs[tx_index]);
        let output_view = cobuild_types::lazy_reader::blockchain::CellOutput::from(output.clone());

        write_count(&mut hasher, local_index);
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
                &mut hasher,
                &output_view.lock().expect("lock").cursor,
                cobuild_core::error::CoreError::MissingHashInput,
            )
            .expect("lock cursor");
        }
        if otx
            .includes_base_output_type(local_index)
            .expect("output mask")
        {
            update_cursor_with_error(
                &mut hasher,
                &output
                    .table_slice_by_index(2)
                    .expect("output type option cursor"),
                cobuild_core::error::CoreError::MissingHashInput,
            )
            .expect("type cursor");
        }
        if otx
            .includes_base_output_data(local_index)
            .expect("output mask")
        {
            hasher.update(&checked_len_prefix(fixture.output_data[tx_index].len()));
            hasher.update(&fixture.output_data[tx_index]);
        }
    }

    write_count(&mut hasher, otx.base_cell_deps);
    write_len_prefixed_bytes(&mut hasher, otx.base_cell_dep_masks.bytes());
    for local_index in 0..otx.base_cell_deps {
        if otx
            .base_cell_dep_masks
            .get(local_index)
            .expect("cell dep mask")
        {
            let tx_index = layout.base_cell_deps.start + local_index;
            write_count(&mut hasher, local_index);
            hasher.update(&fixture.raw_cell_deps[tx_index]);
        }
    }

    write_count(&mut hasher, otx.base_header_deps);
    write_len_prefixed_bytes(&mut hasher, otx.base_header_dep_masks.bytes());
    for local_index in 0..otx.base_header_deps {
        if otx
            .base_header_dep_masks
            .get(local_index)
            .expect("header dep mask")
        {
            let tx_index = layout.base_header_deps.start + local_index;
            write_count(&mut hasher, local_index);
            hasher.update(&fixture.header_deps[tx_index]);
        }
    }

    hasher.finalize(&mut out);
    out
}

pub(super) fn otx_append_hash(parts: &OtxFixtureParts, base_hash: [u8; 32]) -> [u8; 32] {
    let (otx, layout, fixture) = otx_hash_inputs(parts, 0);
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32)
        .personal(OTX_APPEND_PERSONAL)
        .build();

    update_cursor_with_error(
        &mut hasher,
        &otx.message,
        cobuild_core::error::CoreError::MalformedCobuild,
    )
    .expect("message cursor");
    hasher.update(&base_hash);
    write_count(&mut hasher, otx.append_input_cells);
    for local_index in 0..otx.append_input_cells {
        let tx_index = layout.append_inputs.start + local_index;
        write_count(&mut hasher, local_index);
        hasher.update(&fixture.raw_inputs[tx_index]);
        hasher.update(&fixture.resolved_outputs[tx_index]);
        hasher.update(&checked_len_prefix(fixture.resolved_data[tx_index].len()));
        hasher.update(&fixture.resolved_data[tx_index]);
    }

    write_count(&mut hasher, otx.append_output_cells);
    for local_index in 0..otx.append_output_cells {
        let tx_index = layout.append_outputs.start + local_index;
        write_count(&mut hasher, local_index);
        hasher.update(&fixture.raw_outputs[tx_index]);
        hasher.update(&checked_len_prefix(fixture.output_data[tx_index].len()));
        hasher.update(&fixture.output_data[tx_index]);
    }

    write_count(&mut hasher, otx.append_cell_deps);
    for local_index in 0..otx.append_cell_deps {
        let tx_index = layout.append_cell_deps.start + local_index;
        write_count(&mut hasher, local_index);
        hasher.update(&fixture.raw_cell_deps[tx_index]);
    }

    write_count(&mut hasher, otx.append_header_deps);
    for local_index in 0..otx.append_header_deps {
        let tx_index = layout.append_header_deps.start + local_index;
        write_count(&mut hasher, local_index);
        hasher.update(&fixture.header_deps[tx_index]);
    }

    hasher.finalize(&mut out);
    out
}

fn otx_hash_inputs(
    parts: &OtxFixtureParts,
    base_output_start: usize,
) -> (OtxView, OtxLayout, OtxHashFixture) {
    let base_start = parts.start_input;
    let append_start = base_start + parts.base_inputs.len();
    let append_output_start = base_output_start + parts.base_outputs.len();
    let base_cell_dep_start = 1;
    let append_cell_dep_start = base_cell_dep_start + parts.base_cell_deps.len();
    let append_header_dep_start = parts.base_header_deps.len();
    let mut raw_inputs = vec![Vec::new(); parts.input_count];
    let mut resolved_outputs = vec![Vec::new(); parts.input_count];
    let mut resolved_data = vec![Vec::new(); parts.input_count];
    let mut raw_outputs =
        vec![Vec::new(); base_output_start + parts.base_outputs.len() + parts.append_outputs.len()];
    let mut output_data =
        vec![Vec::new(); base_output_start + parts.base_outputs.len() + parts.append_outputs.len()];
    let mut raw_cell_deps =
        vec![
            Vec::new();
            base_cell_dep_start + parts.base_cell_deps.len() + parts.append_cell_deps.len()
        ];
    let mut header_deps =
        vec![[0u8; 32]; parts.base_header_deps.len() + parts.append_header_deps.len()];

    for (offset, input) in parts.base_inputs.iter().enumerate() {
        let index = base_start + offset;
        raw_inputs[index] = input.raw.clone();
        resolved_outputs[index] = input.resolved_output.clone();
        resolved_data[index] = input.data.clone();
    }
    for (offset, input) in parts.append_inputs.iter().enumerate() {
        let index = append_start + offset;
        raw_inputs[index] = input.raw.clone();
        resolved_outputs[index] = input.resolved_output.clone();
        resolved_data[index] = input.data.clone();
    }
    for (offset, output) in parts.base_outputs.iter().enumerate() {
        let index = base_output_start + offset;
        raw_outputs[index] = output.raw.clone();
        output_data[index] = output.data.clone();
    }
    for (offset, output) in parts.append_outputs.iter().enumerate() {
        let index = append_output_start + offset;
        raw_outputs[index] = output.raw.clone();
        output_data[index] = output.data.clone();
    }
    for (offset, cell_dep) in parts.base_cell_deps.iter().enumerate() {
        raw_cell_deps[base_cell_dep_start + offset] = cell_dep.clone();
    }
    for (offset, cell_dep) in parts.append_cell_deps.iter().enumerate() {
        raw_cell_deps[append_cell_dep_start + offset] = cell_dep.clone();
    }
    for (offset, header_dep) in parts.base_header_deps.iter().enumerate() {
        header_deps[offset] = *header_dep;
    }
    for (offset, header_dep) in parts.append_header_deps.iter().enumerate() {
        header_deps[append_header_dep_start + offset] = *header_dep;
    }

    let otx = OtxView {
        message: cursor_from_slice(&parts.message),
        append_permissions: parts.append_permissions,
        base_input_cells: parts.base_inputs.len(),
        base_input_masks: mask_view(&parts.base_input_masks),
        base_output_cells: parts.base_outputs.len(),
        base_output_masks: mask_view(&parts.base_output_masks),
        base_cell_deps: parts.base_cell_deps.len(),
        base_cell_dep_masks: mask_view(&parts.base_cell_dep_masks),
        base_header_deps: parts.base_header_deps.len(),
        base_header_dep_masks: mask_view(&parts.base_header_dep_masks),
        append_input_cells: parts.append_inputs.len(),
        append_output_cells: parts.append_outputs.len(),
        append_cell_deps: parts.append_cell_deps.len(),
        append_header_deps: parts.append_header_deps.len(),
        seals: Vec::new(),
    };
    let layout = OtxLayout {
        witness_index: 0,
        base_inputs: range(base_start, parts.base_inputs.len()),
        append_inputs: range(append_start, parts.append_inputs.len()),
        base_outputs: range(base_output_start, parts.base_outputs.len()),
        append_outputs: range(append_output_start, parts.append_outputs.len()),
        base_cell_deps: range(base_cell_dep_start, parts.base_cell_deps.len()),
        append_cell_deps: range(append_cell_dep_start, parts.append_cell_deps.len()),
        base_header_deps: range(0, parts.base_header_deps.len()),
        append_header_deps: range(append_header_dep_start, parts.append_header_deps.len()),
    };
    let fixture = OtxHashFixture {
        raw_inputs,
        resolved_outputs,
        resolved_data,
        raw_outputs,
        output_data,
        raw_cell_deps,
        header_deps,
    };
    (otx, layout, fixture)
}

fn mask_view(bytes: &[u8]) -> MaskView {
    MaskView::new(bytes.to_vec())
}

fn range(start: usize, count: usize) -> Range {
    Range { start, count }
}
