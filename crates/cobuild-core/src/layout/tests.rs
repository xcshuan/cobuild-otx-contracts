use alloc::{vec, vec::Vec};

use super::*;
use crate::{
    protocol::{
        SegmentFlags, APPEND_PERMISSION_CELL_DEPS_BIT, APPEND_PERMISSION_HEADER_DEPS_BIT,
        APPEND_PERMISSION_INPUTS_BIT, APPEND_PERMISSION_OUTPUTS_BIT, SEGMENT_FLAG_ALLOW_MORE_AFTER,
    },
    reader::cursor_from_slice,
    witness::CobuildWitnessScanner,
};

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
fn duplicate_otx_start_is_rejected() {
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
}

#[test]
fn otx_start_without_following_otx_is_rejected() {
    assert_invalid(
        build_layout(vec![otx_start_witness()], 0, 0, 0, 0),
        CoreError::InvalidOtxLayout,
    );
}

#[test]
fn otx_entity_counts_must_fit_transaction_entity_counts() {
    for (witness, input_count, output_count, cell_dep_count, header_dep_count) in [
        (otx_witness_with_counts(2, 0, 0, 0, 0, 0), 1, 0, 0, 0),
        (otx_witness_with_counts(1, 1, 0, 0, 0, 0), 1, 0, 0, 0),
        (otx_witness_with_counts(1, 0, 2, 0, 0, 0), 1, 1, 0, 0),
        (otx_witness_with_counts(1, 0, 0, 1, 0, 0), 1, 0, 0, 0),
        (otx_witness_with_counts(1, 0, 0, 0, 2, 0), 1, 0, 1, 0),
        (
            otx_witness_with_append_counts(
                append_permissions(&[APPEND_PERMISSION_CELL_DEPS_BIT]),
                0,
                0,
                1,
                0,
            ),
            1,
            0,
            0,
            0,
        ),
        (
            otx_witness_with_append_counts(
                append_permissions(&[APPEND_PERMISSION_HEADER_DEPS_BIT]),
                0,
                0,
                0,
                1,
            ),
            1,
            0,
            0,
            0,
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
            CoreError::InvalidOtxLayout,
        );
    }
}

#[test]
fn append_entity_counts_require_matching_permission_bits() {
    for witness in [
        otx_witness_with_append_counts(0, 1, 0, 0, 0),
        otx_witness_with_append_counts(0, 0, 1, 0, 0),
        otx_witness_with_append_counts(0, 0, 0, 1, 0),
        otx_witness_with_append_counts(0, 0, 0, 0, 1),
    ] {
        assert_invalid(
            build_layout(vec![otx_start_witness(), witness], 2, 1, 1, 1),
            CoreError::InvalidOtxLayout,
        );
    }
}

#[test]
fn base_masks_reject_wrong_lengths() {
    for (witness, input_count, output_count, cell_dep_count, header_dep_count) in [
        (otx_witness_with_base_input_mask(&[]), 1, 0, 0, 0),
        (otx_witness_with_base_input_mask(&[0, 0]), 1, 0, 0, 0),
        (otx_witness_with_base_output_mask(1, &[]), 1, 1, 0, 0),
        (otx_witness_with_base_output_mask(1, &[0, 0]), 1, 1, 0, 0),
        (otx_witness_with_base_cell_dep_mask(1, &[]), 1, 0, 1, 0),
        (otx_witness_with_base_cell_dep_mask(1, &[0, 0]), 1, 0, 1, 0),
        (otx_witness_with_base_header_dep_mask(1, &[]), 1, 0, 0, 1),
        (
            otx_witness_with_base_header_dep_mask(1, &[0, 0]),
            1,
            0,
            0,
            1,
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
            CoreError::InvalidOtxLayout,
        );
    }
}

#[test]
fn base_masks_reject_nonzero_padding_bits() {
    for (witness, input_count, output_count, cell_dep_count, header_dep_count) in [
        (otx_witness_with_base_input_mask(&[0b0000_0100]), 1, 0, 0, 0),
        (
            otx_witness_with_base_output_mask(1, &[0b0001_0000]),
            1,
            1,
            0,
            0,
        ),
        (
            otx_witness_with_base_cell_dep_mask(1, &[0b0000_0010]),
            1,
            0,
            1,
            0,
        ),
        (
            otx_witness_with_base_header_dep_mask(1, &[0b0000_0010]),
            1,
            0,
            0,
            1,
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
            CoreError::InvalidOtxLayout,
        );
    }
}

#[test]
fn otx_view_validation_rejects_invalid_counts_permissions_masks_and_segment_flags() {
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
            otx_witness_with_append_segment_flags(0x04),
            1,
            0,
            0,
            0,
            CoreError::InvalidOtxLayout,
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

#[test]
fn otx_layout_tracks_each_append_segment_range() {
    let layout = build_layout(
        vec![
            otx_start_witness_with_starts(10, 20, 30, 40),
            otx_witness_custom(OtxWitnessCustom {
                append_permissions: append_permissions(&[
                    APPEND_PERMISSION_INPUTS_BIT,
                    APPEND_PERMISSION_OUTPUTS_BIT,
                    APPEND_PERMISSION_CELL_DEPS_BIT,
                    APPEND_PERMISSION_HEADER_DEPS_BIT,
                ]),
                base_inputs: 2,
                base_input_mask: &[0],
                base_outputs: 3,
                base_output_mask: &[0, 0],
                base_cell_deps: 1,
                base_cell_dep_mask: &[0],
                base_header_deps: 1,
                base_header_dep_mask: &[0],
                append_segments: Some(&dynvec(&[
                    append_segment(OtxAppendSegmentCustom {
                        segment_flags: SEGMENT_FLAG_ALLOW_MORE_AFTER,
                        input_cells: 1,
                        output_cells: 2,
                        cell_deps: 0,
                        header_deps: 1,
                    }),
                    append_segment(OtxAppendSegmentCustom {
                        segment_flags: 0,
                        input_cells: 3,
                        output_cells: 0,
                        cell_deps: 2,
                        header_deps: 0,
                    }),
                ])),
                ..OtxWitnessCustom::default()
            }),
        ],
        16,
        25,
        33,
        42,
    )
    .unwrap();

    let entry = &layout.otx_entries[0].layout;
    assert_eq!(
        entry.base_inputs,
        Range {
            start: 10,
            count: 2
        }
    );
    assert_eq!(
        entry.append_inputs,
        Range {
            start: 12,
            count: 4
        }
    );
    assert_eq!(
        entry.base_outputs,
        Range {
            start: 20,
            count: 3
        }
    );
    assert_eq!(
        entry.append_outputs,
        Range {
            start: 23,
            count: 2
        }
    );
    assert_eq!(
        entry.base_cell_deps,
        Range {
            start: 30,
            count: 1
        }
    );
    assert_eq!(
        entry.append_cell_deps,
        Range {
            start: 31,
            count: 2
        }
    );
    assert_eq!(
        entry.base_header_deps,
        Range {
            start: 40,
            count: 1
        }
    );
    assert_eq!(
        entry.append_header_deps,
        Range {
            start: 41,
            count: 1
        }
    );
    assert_eq!(entry.append_segments.len(), 2);
    assert_eq!(
        entry.append_segments[0].flags,
        SegmentFlags::try_from(SEGMENT_FLAG_ALLOW_MORE_AFTER).unwrap()
    );
    assert_eq!(
        entry.append_segments[0].inputs,
        Range {
            start: 12,
            count: 1
        }
    );
    assert_eq!(
        entry.append_segments[0].outputs,
        Range {
            start: 23,
            count: 2
        }
    );
    assert_eq!(
        entry.append_segments[0].cell_deps,
        Range {
            start: 31,
            count: 0
        }
    );
    assert_eq!(
        entry.append_segments[0].header_deps,
        Range {
            start: 41,
            count: 1
        }
    );
    assert_eq!(
        entry.append_segments[1].flags,
        SegmentFlags::try_from(0).unwrap()
    );
    assert_eq!(
        entry.append_segments[1].inputs,
        Range {
            start: 13,
            count: 3
        }
    );
    assert_eq!(
        entry.append_segments[1].outputs,
        Range {
            start: 25,
            count: 0
        }
    );
    assert_eq!(
        entry.append_segments[1].cell_deps,
        Range {
            start: 31,
            count: 2
        }
    );
    assert_eq!(
        entry.append_segments[1].header_deps,
        Range {
            start: 42,
            count: 0
        }
    );
}

#[test]
fn otx_layout_rejects_closed_segment_before_final_segment() {
    assert_invalid(
        build_layout(
            vec![
                otx_start_witness(),
                otx_witness_custom(OtxWitnessCustom {
                    append_permissions: append_permissions(&[APPEND_PERMISSION_INPUTS_BIT]),
                    append_segments: Some(&dynvec(&[
                        append_segment(OtxAppendSegmentCustom {
                            segment_flags: 0,
                            input_cells: 1,
                            output_cells: 0,
                            cell_deps: 0,
                            header_deps: 0,
                        }),
                        append_segment(OtxAppendSegmentCustom {
                            segment_flags: 0,
                            input_cells: 1,
                            output_cells: 0,
                            cell_deps: 0,
                            header_deps: 0,
                        }),
                    ])),
                    ..OtxWitnessCustom::default()
                }),
            ],
            3,
            0,
            0,
            0,
        ),
        CoreError::InvalidOtxLayout,
    );
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
            empty_dynvec(),
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
    let segment = append_segment(OtxAppendSegmentCustom {
        segment_flags: 0,
        input_cells: append_inputs,
        output_cells: append_outputs,
        cell_deps: append_cell_deps,
        header_deps: append_header_deps,
    });
    let append_segments = dynvec(&[segment]);
    otx_witness_custom(OtxWitnessCustom {
        append_permissions,
        append_segments: Some(&append_segments),
        ..OtxWitnessCustom::default()
    })
}

fn otx_witness_with_append_segment_flags(segment_flags: u8) -> Vec<u8> {
    let segment = append_segment(OtxAppendSegmentCustom {
        segment_flags,
        input_cells: 0,
        output_cells: 0,
        cell_deps: 0,
        header_deps: 0,
    });
    let append_segments = dynvec(&[segment]);
    otx_witness_custom(OtxWitnessCustom {
        append_segments: Some(&append_segments),
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
    let segment = append_segment(OtxAppendSegmentCustom {
        segment_flags: 0,
        input_cells: append_inputs,
        output_cells: append_outputs,
        cell_deps: 0,
        header_deps: 0,
    });
    let append_segments = dynvec(&[segment]);
    otx_witness_custom(OtxWitnessCustom {
        base_inputs,
        base_outputs,
        base_cell_deps,
        base_header_deps,
        base_input_mask: &input_mask,
        base_output_mask: &output_mask,
        base_cell_dep_mask: &cell_dep_mask,
        base_header_dep_mask: &header_dep_mask,
        append_segments: Some(&append_segments),
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
    append_segments: Option<&'a [u8]>,
    base_seals: Option<&'a [u8]>,
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
            append_segments: None,
            base_seals: None,
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
            params
                .append_segments
                .map_or_else(empty_dynvec, |append_segments| append_segments.to_vec()),
            params
                .base_seals
                .map_or_else(empty_dynvec, |base_seals| base_seals.to_vec()),
        ]),
    )
}

#[derive(Clone, Copy)]
struct OtxAppendSegmentCustom {
    segment_flags: u8,
    input_cells: u32,
    output_cells: u32,
    cell_deps: u32,
    header_deps: u32,
}

fn append_segment(params: OtxAppendSegmentCustom) -> Vec<u8> {
    table(&[
        vec![params.segment_flags],
        params.input_cells.to_le_bytes().to_vec(),
        params.output_cells.to_le_bytes().to_vec(),
        params.cell_deps.to_le_bytes().to_vec(),
        params.header_deps.to_le_bytes().to_vec(),
        empty_dynvec(),
    ])
}

fn append_permissions(bits: &[u8]) -> u8 {
    bits.iter().fold(0, |permissions, bit| {
        permissions | (1u8.checked_shl(u32::from(*bit)).unwrap_or(0))
    })
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
