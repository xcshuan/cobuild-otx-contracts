use cobuild_core::{
    error::CoreError,
    layout::{build_layout, LayoutTx},
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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

    assert_eq!(result, Err(CoreError::InvalidLayout));
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
    otx_witness_custom(
        append_permissions,
        1,
        &[0],
        0,
        &[],
        0,
        &[],
        0,
        &[],
        0,
        0,
        0,
        0,
    )
}

fn otx_witness_with_append_counts(
    append_permissions: u8,
    append_inputs: u32,
    append_outputs: u32,
    append_cell_deps: u32,
    append_header_deps: u32,
) -> Vec<u8> {
    otx_witness_custom(
        append_permissions,
        1,
        &[0],
        0,
        &[],
        0,
        &[],
        0,
        &[],
        append_inputs,
        append_outputs,
        append_cell_deps,
        append_header_deps,
    )
}

fn otx_witness_with_base_input_mask(mask: &[u8]) -> Vec<u8> {
    otx_witness_custom(0, 1, mask, 0, &[], 0, &[], 0, &[], 0, 0, 0, 0)
}

fn otx_witness_with_base_output_mask(base_outputs: u32, mask: &[u8]) -> Vec<u8> {
    otx_witness_custom(0, 1, &[0], base_outputs, mask, 0, &[], 0, &[], 0, 0, 0, 0)
}

fn otx_witness_with_base_cell_dep_mask(base_cell_deps: u32, mask: &[u8]) -> Vec<u8> {
    otx_witness_custom(0, 1, &[0], 0, &[], base_cell_deps, mask, 0, &[], 0, 0, 0, 0)
}

fn otx_witness_with_base_header_dep_mask(base_header_deps: u32, mask: &[u8]) -> Vec<u8> {
    otx_witness_custom(
        0,
        1,
        &[0],
        0,
        &[],
        0,
        &[],
        base_header_deps,
        mask,
        0,
        0,
        0,
        0,
    )
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
    otx_witness_custom(
        0,
        base_inputs,
        &input_mask,
        base_outputs,
        &output_mask,
        base_cell_deps,
        &cell_dep_mask,
        base_header_deps,
        &header_dep_mask,
        append_inputs,
        append_outputs,
        0,
        0,
    )
}

fn otx_witness_custom(
    append_permissions: u8,
    base_inputs: u32,
    base_input_mask: &[u8],
    base_outputs: u32,
    base_output_mask: &[u8],
    base_cell_deps: u32,
    base_cell_dep_mask: &[u8],
    base_header_deps: u32,
    base_header_dep_mask: &[u8],
    append_inputs: u32,
    append_outputs: u32,
    append_cell_deps: u32,
    append_header_deps: u32,
) -> Vec<u8> {
    witness_union(
        0xff00_0003,
        &table(&[
            empty_message(),
            vec![append_permissions],
            base_inputs.to_le_bytes().to_vec(),
            molecule_bytes(base_input_mask),
            base_outputs.to_le_bytes().to_vec(),
            molecule_bytes(base_output_mask),
            base_cell_deps.to_le_bytes().to_vec(),
            molecule_bytes(base_cell_dep_mask),
            base_header_deps.to_le_bytes().to_vec(),
            molecule_bytes(base_header_dep_mask),
            append_inputs.to_le_bytes().to_vec(),
            append_outputs.to_le_bytes().to_vec(),
            append_cell_deps.to_le_bytes().to_vec(),
            append_header_deps.to_le_bytes().to_vec(),
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
