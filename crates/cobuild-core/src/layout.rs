use alloc::vec::Vec;

use crate::error::CoreError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutTx {
    pub witnesses: Vec<Vec<u8>>,
    pub input_count: usize,
    pub output_count: usize,
    pub cell_dep_count: usize,
    pub header_dep_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Range {
    pub start: usize,
    pub count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OtxLayout {
    pub witness_index: usize,
    pub base_inputs: Range,
    pub append_inputs: Range,
    pub base_outputs: Range,
    pub append_outputs: Range,
    pub base_cell_deps: Range,
    pub append_cell_deps: Range,
    pub base_header_deps: Range,
    pub append_header_deps: Range,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltLayout {
    pub otxs: Vec<OtxLayout>,
}

pub fn build_layout(tx: &LayoutTx) -> Result<BuiltLayout, CoreError> {
    if tx.witnesses.is_empty() {
        return Ok(BuiltLayout { otxs: Vec::new() });
    }

    Ok(BuiltLayout { otxs: Vec::new() })
}
