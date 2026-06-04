use cobuild_core::{
    error::CoreError,
    layout::{build_layout, build_layout_from_witnesses, LayoutTx},
};

#[test]
fn empty_tx_has_no_otx_layouts() {
    let layout = build_layout(&LayoutTx {
        witnesses: Vec::new(),
        input_count: 0,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    })
    .unwrap();
    assert!(layout.otxs.is_empty());
}

#[test]
fn otx_without_start_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_witness()],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn otx_witnesses_must_be_contiguous_after_start() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![
            otx_start_witness(),
            otx_witness(),
            Vec::new(),
            otx_witness(),
        ],
        input_count: 2,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn duplicate_otx_start_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_start_witness(), otx_witness()],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn otx_start_without_following_otx_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness()],
        input_count: 0,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn zero_base_inputs_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![
            otx_start_witness(),
            otx_witness_with_counts(0, 0, 0, 0, 0, 0),
        ],
        input_count: 0,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn reserved_append_permission_bits_are_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_permissions(0x10)],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn append_count_without_permission_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![
            otx_start_witness(),
            otx_witness_with_append_counts(0, 1, 0, 0, 0),
        ],
        input_count: 2,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn append_output_without_permission_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![
            otx_start_witness(),
            otx_witness_with_append_counts(0, 0, 1, 0, 0),
        ],
        input_count: 1,
        output_count: 1,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn append_cell_dep_without_permission_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![
            otx_start_witness(),
            otx_witness_with_append_counts(0, 0, 0, 1, 0),
        ],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 1,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn append_header_dep_without_permission_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![
            otx_start_witness(),
            otx_witness_with_append_counts(0, 0, 0, 0, 1),
        ],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 1,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn invalid_base_input_mask_length_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_base_input_mask(&[])],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn invalid_base_output_mask_length_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![
            otx_start_witness(),
            otx_witness_with_base_output_mask(1, &[]),
        ],
        input_count: 1,
        output_count: 1,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn invalid_base_cell_dep_mask_length_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![
            otx_start_witness(),
            otx_witness_with_base_cell_dep_mask(1, &[]),
        ],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 1,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn invalid_base_header_dep_mask_length_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![
            otx_start_witness(),
            otx_witness_with_base_header_dep_mask(1, &[]),
        ],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 1,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn non_zero_base_input_mask_padding_bits_are_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![
            otx_start_witness(),
            otx_witness_with_base_input_mask(&[0b0000_0100]),
        ],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn non_zero_base_output_mask_padding_bits_are_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![
            otx_start_witness(),
            otx_witness_with_base_output_mask(1, &[0b0001_0000]),
        ],
        input_count: 1,
        output_count: 1,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn non_zero_base_cell_dep_mask_padding_bits_are_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![
            otx_start_witness(),
            otx_witness_with_base_cell_dep_mask(1, &[0b0000_0010]),
        ],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 1,
        header_dep_count: 0,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn non_zero_base_header_dep_mask_padding_bits_are_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![
            otx_start_witness(),
            otx_witness_with_base_header_dep_mask(1, &[0b0000_0010]),
        ],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 1,
    });

    assert_invalid(result, CoreError::InvalidOtxLayout);
}

#[test]
fn direct_witness_layout_matches_owned_layout() {
    let witnesses = vec![otx_start_witness(), otx_witness()];
    let tx = LayoutTx {
        witnesses: witnesses.clone(),
        ..LayoutTx::default()
    };

    let direct_layout = build_layout_from_witnesses(&tx, 1, 0, 0, 0).unwrap();
    let owned_layout = build_layout(&LayoutTx {
        witnesses,
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    })
    .unwrap();

    assert_eq!(direct_layout.otxs, owned_layout.otxs);
}

fn otx_start_witness() -> Vec<u8> {
    witness_union(
        0xff00_0004,
        &table(&[
            0u32.to_le_bytes().to_vec(),
            0u32.to_le_bytes().to_vec(),
            0u32.to_le_bytes().to_vec(),
            0u32.to_le_bytes().to_vec(),
        ]),
    )
}

fn assert_invalid<T>(result: Result<T, CoreError>, expected: CoreError) {
    assert_eq!(result.err(), Some(expected));
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
            empty_dynvec(),
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

fn molecule_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
    out
}
