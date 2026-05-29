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
