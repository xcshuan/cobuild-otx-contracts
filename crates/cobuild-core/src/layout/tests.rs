use alloc::{vec, vec::Vec};

use super::*;
use crate::{reader::cursor_from_slice, witness::CobuildWitnessScanner};

#[test]
fn empty_tx_has_no_otx_layouts() {
    let layout = build_layout(Vec::new(), 0, 0, 0, 0).unwrap();

    assert!(layout.otx_entries.is_empty());
}

#[test]
fn otx_sequence_rules_reject_invalid_shapes() {
    assert_invalid(
        build_layout(vec![otx_witness()], 1, 0, 0, 0),
        CoreError::InvalidOtxLayout,
    );
    assert_invalid(
        build_layout(
            vec![
                otx_start_witness(),
                otx_witness(),
                Vec::new(),
                otx_witness(),
            ],
            2,
            0,
            0,
            0,
        ),
        CoreError::InvalidOtxLayout,
    );
    assert_invalid(
        build_layout(
            vec![otx_start_witness(), otx_start_witness(), otx_witness()],
            1,
            0,
            0,
            0,
        ),
        CoreError::InvalidOtxLayout,
    );
    assert_invalid(
        build_layout(vec![otx_start_witness()], 0, 0, 0, 0),
        CoreError::InvalidOtxLayout,
    );
}

#[test]
fn otx_view_validation_rejects_invalid_counts_permissions_masks_and_seals() {
    for (witness, input_count, output_count, cell_dep_count, header_dep_count, error) in [
        (
            otx_witness_with_counts(0, 0, 0, 0, 0, 0),
            0,
            0,
            0,
            0,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_permissions(0x10),
            1,
            0,
            0,
            0,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_append_counts(0, 1, 0, 0, 0),
            2,
            0,
            0,
            0,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_append_counts(0, 0, 1, 0, 0),
            1,
            1,
            0,
            0,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_append_counts(0, 0, 0, 1, 0),
            1,
            0,
            1,
            0,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_append_counts(0, 0, 0, 0, 1),
            1,
            0,
            0,
            1,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_base_input_mask(&[]),
            1,
            0,
            0,
            0,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_base_output_mask(1, &[]),
            1,
            1,
            0,
            0,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_base_cell_dep_mask(1, &[]),
            1,
            0,
            1,
            0,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_base_header_dep_mask(1, &[]),
            1,
            0,
            0,
            1,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_base_input_mask(&[0b0000_0100]),
            1,
            0,
            0,
            0,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_base_output_mask(1, &[0b0001_0000]),
            1,
            1,
            0,
            0,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_base_cell_dep_mask(1, &[0b0000_0010]),
            1,
            0,
            1,
            0,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_base_header_dep_mask(1, &[0b0000_0010]),
            1,
            0,
            0,
            1,
            CoreError::InvalidOtxLayout,
        ),
        (
            otx_witness_with_seal_scope(9),
            1,
            0,
            0,
            0,
            CoreError::InvalidSealScope,
        ),
    ] {
        assert_invalid(
            build_layout(
                vec![otx_start_witness(), witness],
                input_count,
                output_count,
                cell_dep_count,
                header_dep_count,
            ),
            error,
        );
    }
}

#[test]
fn legacy_witness_after_otx_sequence_is_allowed() {
    let layout = build_layout(
        vec![otx_start_witness(), otx_witness(), vec![0, 1, 2, 3]],
        1,
        0,
        0,
        0,
    )
    .unwrap();

    assert_eq!(layout.otx_entries.len(), 1);
}

#[test]
fn built_layout_tracks_aggregate_input_and_output_ranges() {
    let layout = build_layout(
        vec![
            otx_start_witness_with_starts(2, 3, 0, 0),
            otx_witness_with_counts(1, 0, 1, 0, 0, 0),
            otx_witness_with_counts(2, 0, 2, 0, 0, 0),
        ],
        5,
        6,
        0,
        0,
    )
    .unwrap();

    assert_eq!(layout.input_range, Range { start: 2, count: 3 });
    assert_eq!(layout.output_range, Range { start: 3, count: 3 });
}

fn build_layout(
    witnesses: Vec<Vec<u8>>,
    input_count: usize,
    output_count: usize,
    cell_dep_count: usize,
    header_dep_count: usize,
) -> Result<BuiltLayout, CoreError> {
    let mut scanner = CobuildWitnessScanner::with_capacity(witnesses.len());
    for witness in witnesses {
        scanner.push_witness(cursor_from_slice(&witness))?;
    }
    match scanner
        .finish(input_count, output_count, cell_dep_count, header_dep_count)?
        .otx_layouts
    {
        OtxLayouts::None => Ok(BuiltLayout {
            input_range: Range { start: 0, count: 0 },
            output_range: Range { start: 0, count: 0 },
            otx_entries: Vec::new(),
        }),
        OtxLayouts::Complete(layout) => Ok(layout),
    }
}

fn assert_invalid<T>(result: Result<T, CoreError>, expected: CoreError) {
    assert_eq!(result.err(), Some(expected));
}

fn otx_start_witness() -> Vec<u8> {
    otx_start_witness_with_starts(0, 0, 0, 0)
}

fn otx_start_witness_with_starts(
    input_start: u32,
    output_start: u32,
    cell_dep_start: u32,
    header_dep_start: u32,
) -> Vec<u8> {
    witness_union(
        0xff00_0004,
        &table(&[
            input_start.to_le_bytes().to_vec(),
            output_start.to_le_bytes().to_vec(),
            cell_dep_start.to_le_bytes().to_vec(),
            header_dep_start.to_le_bytes().to_vec(),
        ]),
    )
}

fn otx_witness() -> Vec<u8> {
    witness_union(
        0xff00_0003,
        &table(&[
            empty_message(),
            vec![0],
            1u32.to_le_bytes().to_vec(),
            molecule_bytes(&[0]),
            0u32.to_le_bytes().to_vec(),
            molecule_bytes(&[]),
            0u32.to_le_bytes().to_vec(),
            molecule_bytes(&[]),
            0u32.to_le_bytes().to_vec(),
            molecule_bytes(&[]),
            0u32.to_le_bytes().to_vec(),
            0u32.to_le_bytes().to_vec(),
            0u32.to_le_bytes().to_vec(),
            0u32.to_le_bytes().to_vec(),
            empty_dynvec(),
        ]),
    )
}

fn otx_witness_with_permissions(append_permissions: u8) -> Vec<u8> {
    otx_witness_custom(OtxWitnessCustom {
        append_permissions,
        ..OtxWitnessCustom::default()
    })
}

fn otx_witness_with_append_counts(
    append_permissions: u8,
    append_inputs: u32,
    append_outputs: u32,
    append_cell_deps: u32,
    append_header_deps: u32,
) -> Vec<u8> {
    otx_witness_custom(OtxWitnessCustom {
        append_permissions,
        append_inputs,
        append_outputs,
        append_cell_deps,
        append_header_deps,
        ..OtxWitnessCustom::default()
    })
}

fn otx_witness_with_base_input_mask(mask: &[u8]) -> Vec<u8> {
    otx_witness_custom(OtxWitnessCustom {
        base_input_mask: mask,
        ..OtxWitnessCustom::default()
    })
}

fn otx_witness_with_base_output_mask(base_outputs: u32, mask: &[u8]) -> Vec<u8> {
    otx_witness_custom(OtxWitnessCustom {
        base_outputs,
        base_output_mask: mask,
        ..OtxWitnessCustom::default()
    })
}

fn otx_witness_with_base_cell_dep_mask(base_cell_deps: u32, mask: &[u8]) -> Vec<u8> {
    otx_witness_custom(OtxWitnessCustom {
        base_cell_deps,
        base_cell_dep_mask: mask,
        ..OtxWitnessCustom::default()
    })
}

fn otx_witness_with_base_header_dep_mask(base_header_deps: u32, mask: &[u8]) -> Vec<u8> {
    otx_witness_custom(OtxWitnessCustom {
        base_header_deps,
        base_header_dep_mask: mask,
        ..OtxWitnessCustom::default()
    })
}

fn otx_witness_with_seal_scope(scope: u8) -> Vec<u8> {
    let seal_pair = table(&[vec![0u8; 32], vec![scope], molecule_bytes(&[0x11, 0x22])]);
    let seals = dynvec(&[seal_pair]);
    otx_witness_custom(OtxWitnessCustom {
        seals: Some(&seals),
        ..OtxWitnessCustom::default()
    })
}

fn otx_witness_with_counts(
    base_inputs: u32,
    append_inputs: u32,
    base_outputs: u32,
    append_outputs: u32,
    base_cell_deps: u32,
    base_header_deps: u32,
) -> Vec<u8> {
    let input_mask = vec![0; ((base_inputs as usize) * 2).div_ceil(8)];
    let output_mask = vec![0; ((base_outputs as usize) * 4).div_ceil(8)];
    let cell_dep_mask = vec![0; (base_cell_deps as usize).div_ceil(8)];
    let header_dep_mask = vec![0; (base_header_deps as usize).div_ceil(8)];
    otx_witness_custom(OtxWitnessCustom {
        base_inputs,
        base_outputs,
        base_cell_deps,
        base_header_deps,
        append_inputs,
        append_outputs,
        base_input_mask: &input_mask,
        base_output_mask: &output_mask,
        base_cell_dep_mask: &cell_dep_mask,
        base_header_dep_mask: &header_dep_mask,
        ..OtxWitnessCustom::default()
    })
}

struct OtxWitnessCustom<'a> {
    append_permissions: u8,
    base_inputs: u32,
    base_input_mask: &'a [u8],
    base_outputs: u32,
    base_output_mask: &'a [u8],
    base_cell_deps: u32,
    base_cell_dep_mask: &'a [u8],
    base_header_deps: u32,
    base_header_dep_mask: &'a [u8],
    append_inputs: u32,
    append_outputs: u32,
    append_cell_deps: u32,
    append_header_deps: u32,
    seals: Option<&'a [u8]>,
}

impl Default for OtxWitnessCustom<'_> {
    fn default() -> Self {
        Self {
            append_permissions: 0,
            base_inputs: 1,
            base_input_mask: &[0],
            base_outputs: 0,
            base_output_mask: &[],
            base_cell_deps: 0,
            base_cell_dep_mask: &[],
            base_header_deps: 0,
            base_header_dep_mask: &[],
            append_inputs: 0,
            append_outputs: 0,
            append_cell_deps: 0,
            append_header_deps: 0,
            seals: None,
        }
    }
}

fn otx_witness_custom(params: OtxWitnessCustom<'_>) -> Vec<u8> {
    witness_union(
        0xff00_0003,
        &table(&[
            empty_message(),
            vec![params.append_permissions],
            params.base_inputs.to_le_bytes().to_vec(),
            molecule_bytes(params.base_input_mask),
            params.base_outputs.to_le_bytes().to_vec(),
            molecule_bytes(params.base_output_mask),
            params.base_cell_deps.to_le_bytes().to_vec(),
            molecule_bytes(params.base_cell_dep_mask),
            params.base_header_deps.to_le_bytes().to_vec(),
            molecule_bytes(params.base_header_dep_mask),
            params.append_inputs.to_le_bytes().to_vec(),
            params.append_outputs.to_le_bytes().to_vec(),
            params.append_cell_deps.to_le_bytes().to_vec(),
            params.append_header_deps.to_le_bytes().to_vec(),
            params
                .seals
                .map_or_else(empty_dynvec, |seals| seals.to_vec()),
        ]),
    )
}

fn empty_message() -> Vec<u8> {
    table(&[empty_dynvec()])
}

fn witness_union(item_id: u32, item: &[u8]) -> Vec<u8> {
    let mut witness = Vec::with_capacity(4 + item.len());
    witness.extend_from_slice(&item_id.to_le_bytes());
    witness.extend_from_slice(item);
    witness
}

fn table(fields: &[Vec<u8>]) -> Vec<u8> {
    let header_size = 4 + fields.len() * 4;
    let total_size = header_size + fields.iter().map(Vec::len).sum::<usize>();
    let mut out = Vec::with_capacity(total_size);
    out.extend_from_slice(&(total_size as u32).to_le_bytes());
    let mut offset = header_size;
    for field in fields {
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += field.len();
    }
    for field in fields {
        out.extend_from_slice(field);
    }
    out
}

fn empty_dynvec() -> Vec<u8> {
    4u32.to_le_bytes().to_vec()
}

fn dynvec(items: &[Vec<u8>]) -> Vec<u8> {
    if items.is_empty() {
        return empty_dynvec();
    }

    let header_size = 4 + items.len() * 4;
    let total_size = header_size + items.iter().map(Vec::len).sum::<usize>();
    let mut out = Vec::with_capacity(total_size);
    out.extend_from_slice(&(total_size as u32).to_le_bytes());
    let mut offset = header_size;
    for item in items {
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += item.len();
    }
    for item in items {
        out.extend_from_slice(item);
    }
    out
}

fn molecule_bytes(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + data.len());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    out
}
