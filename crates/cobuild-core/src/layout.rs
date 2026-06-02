use alloc::vec::Vec;

use crate::{
    error::CoreError,
    protocol::AppendPermissions,
    view::{OtxStartData, WitnessLayoutView},
};

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
pub struct OtxLayoutData {
    pub layout: OtxLayout,
    pub witness: crate::view::OtxData,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltLayout {
    pub otxs: Vec<OtxLayout>,
    pub otx_data: Vec<OtxLayoutData>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OtxLayoutScan {
    None,
    Complete(BuiltLayout),
    Invalid {
        anchor: Option<OtxStartData>,
        error: CoreError,
    },
}

pub fn build_layout(tx: &LayoutTx) -> Result<BuiltLayout, CoreError> {
    match scan_layout(tx) {
        OtxLayoutScan::None => Ok(empty_layout()),
        OtxLayoutScan::Complete(layout) => Ok(layout),
        OtxLayoutScan::Invalid { error, .. } => Err(error),
    }
}

pub fn scan_layout(tx: &LayoutTx) -> OtxLayoutScan {
    if tx.witnesses.is_empty() {
        return OtxLayoutScan::None;
    }

    let mut start = None;
    let mut last_otx_or_start = None;
    for (index, witness) in tx.witnesses.iter().enumerate() {
        if witness.is_empty() {
            continue;
        }
        let Ok(view) = WitnessLayoutView::from_slice(witness) else {
            continue;
        };
        let otx_start = match view.otx_start() {
            Ok(data) => data,
            Err(error) => return invalid_layout(None, error),
        };
        if let Some(data) = otx_start {
            if start.is_some() {
                return invalid_layout(
                    start.as_ref().map(|(_, data)| data),
                    CoreError::InvalidOtxLayout,
                );
            }
            start = Some((index, data));
            last_otx_or_start = Some(index);
            continue;
        }

        let otx = match view.otx() {
            Ok(data) => data,
            Err(error) => return invalid_layout(None, error),
        };
        if otx.is_some() {
            let Some(last_index) = last_otx_or_start else {
                return invalid_layout(None, CoreError::InvalidOtxLayout);
            };
            if last_index + 1 != index {
                return invalid_layout(None, CoreError::InvalidOtxLayout);
            }
            last_otx_or_start = Some(index);
        }
    }

    let Some((start_witness_index, start_data)) = start else {
        return OtxLayoutScan::None;
    };
    if last_otx_or_start == Some(start_witness_index) {
        return invalid_layout(Some(&start_data), CoreError::InvalidOtxLayout);
    }

    let mut next_input = start_data.start_input_cell;
    let mut next_output = start_data.start_output_cell;
    let mut next_cell_dep = start_data.start_cell_deps;
    let mut next_header_dep = start_data.start_header_deps;
    let mut otxs = Vec::new();
    let mut otx_data = Vec::new();

    for witness_index in (start_witness_index + 1)..tx.witnesses.len() {
        let witness = &tx.witnesses[witness_index];
        if witness.is_empty() {
            break;
        }
        let view = match WitnessLayoutView::from_slice(witness) {
            Ok(view) => view,
            Err(error) => return invalid_layout(Some(&start_data), error),
        };
        let data = match view.otx() {
            Ok(data) => data,
            Err(error) => return invalid_layout(Some(&start_data), error),
        };
        let Some(data) = data else {
            break;
        };
        if let Err(error) = validate_otx_data(&data) {
            return invalid_layout(Some(&start_data), error);
        }

        let base_inputs = match take_range(&mut next_input, data.base_input_cells) {
            Ok(range) => range,
            Err(error) => return invalid_layout(Some(&start_data), error),
        };
        let append_inputs = match take_range(&mut next_input, data.append_input_cells) {
            Ok(range) => range,
            Err(error) => return invalid_layout(Some(&start_data), error),
        };
        let base_outputs = match take_range(&mut next_output, data.base_output_cells) {
            Ok(range) => range,
            Err(error) => return invalid_layout(Some(&start_data), error),
        };
        let append_outputs = match take_range(&mut next_output, data.append_output_cells) {
            Ok(range) => range,
            Err(error) => return invalid_layout(Some(&start_data), error),
        };
        let base_cell_deps = match take_range(&mut next_cell_dep, data.base_cell_deps) {
            Ok(range) => range,
            Err(error) => return invalid_layout(Some(&start_data), error),
        };
        let append_cell_deps = match take_range(&mut next_cell_dep, data.append_cell_deps) {
            Ok(range) => range,
            Err(error) => return invalid_layout(Some(&start_data), error),
        };
        let base_header_deps = match take_range(&mut next_header_dep, data.base_header_deps) {
            Ok(range) => range,
            Err(error) => return invalid_layout(Some(&start_data), error),
        };
        let append_header_deps = match take_range(&mut next_header_dep, data.append_header_deps) {
            Ok(range) => range,
            Err(error) => return invalid_layout(Some(&start_data), error),
        };

        let layout = OtxLayout {
            witness_index,
            base_inputs,
            append_inputs,
            base_outputs,
            append_outputs,
            base_cell_deps,
            append_cell_deps,
            base_header_deps,
            append_header_deps,
        };
        otxs.push(layout.clone());
        otx_data.push(OtxLayoutData {
            layout,
            witness: data,
        });
    }

    if otxs.is_empty() {
        return invalid_layout(Some(&start_data), CoreError::InvalidOtxLayout);
    }
    if let Err(error) = ensure_within(next_input, tx.input_count) {
        return invalid_layout(Some(&start_data), error);
    }
    if let Err(error) = ensure_within(next_output, tx.output_count) {
        return invalid_layout(Some(&start_data), error);
    }
    if let Err(error) = ensure_within(next_cell_dep, tx.cell_dep_count) {
        return invalid_layout(Some(&start_data), error);
    }
    if let Err(error) = ensure_within(next_header_dep, tx.header_dep_count) {
        return invalid_layout(Some(&start_data), error);
    }

    OtxLayoutScan::Complete(BuiltLayout { otxs, otx_data })
}

fn invalid_layout(anchor: Option<&OtxStartData>, error: CoreError) -> OtxLayoutScan {
    OtxLayoutScan::Invalid {
        anchor: anchor.cloned(),
        error,
    }
}

fn empty_layout() -> BuiltLayout {
    BuiltLayout {
        otxs: Vec::new(),
        otx_data: Vec::new(),
    }
}

fn take_range(start: &mut usize, count: usize) -> Result<Range, CoreError> {
    let range = Range {
        start: *start,
        count,
    };
    *start = start
        .checked_add(count)
        .ok_or(CoreError::InvalidOtxLayout)?;
    Ok(range)
}

fn ensure_within(value: usize, max: usize) -> Result<(), CoreError> {
    if value <= max {
        Ok(())
    } else {
        Err(CoreError::InvalidOtxLayout)
    }
}

fn validate_otx_data(data: &crate::view::OtxData) -> Result<(), CoreError> {
    if data.base_input_cells == 0 {
        return Err(CoreError::InvalidOtxLayout);
    }
    let append_permissions = AppendPermissions::try_from(data.append_permissions)?;
    append_permissions.require_allowed(0, data.append_input_cells)?;
    append_permissions.require_allowed(1, data.append_output_cells)?;
    append_permissions.require_allowed(2, data.append_cell_deps)?;
    append_permissions.require_allowed(3, data.append_header_deps)?;
    validate_mask(&data.base_input_masks, data.base_input_cells * 2)?;
    validate_mask(&data.base_output_masks, data.base_output_cells * 4)?;
    validate_mask(&data.base_cell_dep_masks, data.base_cell_deps)?;
    validate_mask(&data.base_header_dep_masks, data.base_header_deps)?;
    Ok(())
}

fn validate_mask(mask: &[u8], bit_count: usize) -> Result<(), CoreError> {
    let expected_len = bit_count.div_ceil(8);
    if mask.len() != expected_len {
        return Err(CoreError::InvalidOtxLayout);
    }
    if bit_count == 0 {
        return Ok(());
    }
    let used_bits = bit_count % 8;
    if used_bits == 0 {
        return Ok(());
    }
    let allowed = (1u8 << used_bits) - 1;
    if mask[mask.len() - 1] & !allowed != 0 {
        return Err(CoreError::InvalidOtxLayout);
    }
    Ok(())
}
