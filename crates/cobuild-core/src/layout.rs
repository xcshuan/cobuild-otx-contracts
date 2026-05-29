use alloc::vec::Vec;

use crate::{error::CoreError, view::WitnessLayoutView};

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

pub fn build_layout(tx: &LayoutTx) -> Result<BuiltLayout, CoreError> {
    if tx.witnesses.is_empty() {
        return Ok(empty_layout());
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
        if let Some(data) = view.otx_start()? {
            if start.is_some() {
                return Err(CoreError::InvalidLayout);
            }
            start = Some((index, data));
            last_otx_or_start = Some(index);
        } else if view.otx()?.is_some() {
            let Some(last_index) = last_otx_or_start else {
                return Err(CoreError::InvalidLayout);
            };
            if last_index + 1 != index {
                return Err(CoreError::InvalidLayout);
            }
            last_otx_or_start = Some(index);
        }
    }

    let Some((start_witness_index, start_data)) = start else {
        return Ok(empty_layout());
    };
    if last_otx_or_start == Some(start_witness_index) {
        return Err(CoreError::InvalidLayout);
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
        let view = WitnessLayoutView::from_slice(witness)?;
        let Some(data) = view.otx()? else {
            break;
        };
        validate_otx_data(&data)?;

        let base_inputs = take_range(&mut next_input, data.base_input_cells)?;
        let append_inputs = take_range(&mut next_input, data.append_input_cells)?;
        let base_outputs = take_range(&mut next_output, data.base_output_cells)?;
        let append_outputs = take_range(&mut next_output, data.append_output_cells)?;
        let base_cell_deps = take_range(&mut next_cell_dep, data.base_cell_deps)?;
        let append_cell_deps = take_range(&mut next_cell_dep, data.append_cell_deps)?;
        let base_header_deps = take_range(&mut next_header_dep, data.base_header_deps)?;
        let append_header_deps = take_range(&mut next_header_dep, data.append_header_deps)?;

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
        return Err(CoreError::InvalidLayout);
    }
    ensure_within(next_input, tx.input_count)?;
    ensure_within(next_output, tx.output_count)?;
    ensure_within(next_cell_dep, tx.cell_dep_count)?;
    ensure_within(next_header_dep, tx.header_dep_count)?;

    Ok(BuiltLayout { otxs, otx_data })
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
    *start = start.checked_add(count).ok_or(CoreError::InvalidLayout)?;
    Ok(range)
}

fn ensure_within(value: usize, max: usize) -> Result<(), CoreError> {
    if value <= max {
        Ok(())
    } else {
        Err(CoreError::InvalidLayout)
    }
}

fn validate_otx_data(data: &crate::view::OtxData) -> Result<(), CoreError> {
    if data.base_input_cells == 0 {
        return Err(CoreError::InvalidLayout);
    }
    if data.append_permissions & 0xf0 != 0 {
        return Err(CoreError::InvalidLayout);
    }
    validate_append_permission(data.append_permissions, 0, data.append_input_cells)?;
    validate_append_permission(data.append_permissions, 1, data.append_output_cells)?;
    validate_append_permission(data.append_permissions, 2, data.append_cell_deps)?;
    validate_append_permission(data.append_permissions, 3, data.append_header_deps)?;
    validate_mask(&data.base_input_masks, data.base_input_cells * 2)?;
    validate_mask(&data.base_output_masks, data.base_output_cells * 4)?;
    validate_mask(&data.base_cell_dep_masks, data.base_cell_deps)?;
    validate_mask(&data.base_header_dep_masks, data.base_header_deps)?;
    Ok(())
}

fn validate_append_permission(permissions: u8, bit: u8, count: usize) -> Result<(), CoreError> {
    if count > 0 && permissions & (1 << bit) == 0 {
        Err(CoreError::InvalidLayout)
    } else {
        Ok(())
    }
}

fn validate_mask(mask: &[u8], bit_count: usize) -> Result<(), CoreError> {
    let expected_len = bit_count.div_ceil(8);
    if mask.len() != expected_len {
        return Err(CoreError::InvalidLayout);
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
        return Err(CoreError::InvalidLayout);
    }
    Ok(())
}
