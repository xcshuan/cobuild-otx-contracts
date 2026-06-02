use alloc::vec::Vec;

use blake2b_ref::Blake2bBuilder;
use cobuild_types::lazy_reader::blockchain::{CellInput, CellOutput};

use crate::{
    error::CoreError,
    layout::{OtxLayout, Range},
    view::{cursor_from_slice, update_cursor, OtxData},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SigningHashParts {
    pub tx_hash: [u8; 32],
    pub resolved_inputs: Vec<ResolvedInputHashPart>,
    pub trailing_witnesses: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInputHashPart {
    pub output: Vec<u8>,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RawTxParts {
    pub inputs: Vec<Vec<u8>>,
    pub outputs: Vec<Vec<u8>>,
    pub outputs_data: Vec<Vec<u8>>,
    pub cell_deps: Vec<Vec<u8>>,
    pub header_deps: Vec<[u8; 32]>,
}

pub fn tx_without_message_hash(parts: &SigningHashParts) -> Result<[u8; 32], CoreError> {
    tx_signing_hash(b"ckbcb_tnm_core1\0", None, parts)
}

pub fn tx_with_message_hash(
    message: &[u8],
    parts: &SigningHashParts,
) -> Result<[u8; 32], CoreError> {
    tx_signing_hash(b"ckbcb_twm_core1\0", Some(message), parts)
}

fn tx_signing_hash(
    personalization: &[u8; 16],
    message: Option<&[u8]>,
    parts: &SigningHashParts,
) -> Result<[u8; 32], CoreError> {
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32).personal(personalization).build();

    if let Some(message) = message {
        hasher.update(message);
    }
    hasher.update(&parts.tx_hash);
    for input in &parts.resolved_inputs {
        hasher.update(&input.output);
        update_len_prefixed(&mut hasher, &input.data)?;
    }
    for witness in &parts.trailing_witnesses {
        update_len_prefixed(&mut hasher, witness)?;
    }
    hasher.finalize(&mut out);

    Ok(out)
}

pub fn checked_len_prefix(len: usize) -> Result<[u8; 4], CoreError> {
    let len = u32::try_from(len).map_err(|_| CoreError::HashInputTooLarge)?;
    Ok(len.to_le_bytes())
}

pub fn otx_base_hash(
    otx: &OtxData,
    layout: &OtxLayout,
    raw: &RawTxParts,
    resolved_inputs: &[ResolvedInputHashPart],
) -> Result<[u8; 32], CoreError> {
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32)
        .personal(b"ckbcb_otb_core1\0")
        .build();

    hasher.update(&otx.message);
    hasher.update(&[otx.append_permissions]);
    update_count(&mut hasher, otx.base_input_cells)?;
    update_len_prefixed(&mut hasher, &otx.base_input_masks)?;
    for local_index in 0..otx.base_input_cells {
        let tx_index = checked_index(layout.base_inputs, local_index)?;
        let input = raw
            .inputs
            .get(tx_index)
            .ok_or(CoreError::MissingHashInput)?;
        let resolved = resolved_inputs
            .get(tx_index)
            .ok_or(CoreError::MissingHashInput)?;
        let input_view = CellInput::from(cursor_from_slice(input));

        update_count(&mut hasher, local_index)?;
        if mask_bit(&otx.base_input_masks, local_index * 2)? {
            hasher.update(
                &input_view
                    .since()
                    .map_err(|_| CoreError::MalformedCobuild)?
                    .to_le_bytes(),
            );
        }
        if mask_bit(&otx.base_input_masks, local_index * 2 + 1)? {
            let previous_output = input_view
                .previous_output()
                .map_err(|_| CoreError::MalformedCobuild)?;
            update_cursor(&mut hasher, &previous_output.cursor)?;
        }
        hasher.update(&resolved.output);
        update_len_prefixed(&mut hasher, &resolved.data)?;
    }

    update_count(&mut hasher, otx.base_output_cells)?;
    update_len_prefixed(&mut hasher, &otx.base_output_masks)?;
    for local_index in 0..otx.base_output_cells {
        let tx_index = checked_index(layout.base_outputs, local_index)?;
        let output = raw
            .outputs
            .get(tx_index)
            .ok_or(CoreError::MissingHashInput)?;
        let output_data = raw
            .outputs_data
            .get(tx_index)
            .ok_or(CoreError::MissingHashInput)?;
        let output_view = CellOutput::from(cursor_from_slice(output));

        update_count(&mut hasher, local_index)?;
        if mask_bit(&otx.base_output_masks, local_index * 4)? {
            hasher.update(
                &output_view
                    .capacity()
                    .map_err(|_| CoreError::MalformedCobuild)?
                    .to_le_bytes(),
            );
        }
        if mask_bit(&otx.base_output_masks, local_index * 4 + 1)? {
            let lock = output_view
                .lock()
                .map_err(|_| CoreError::MalformedCobuild)?;
            update_cursor(&mut hasher, &lock.cursor)?;
        }
        if mask_bit(&otx.base_output_masks, local_index * 4 + 2)? {
            let type_cursor = output_view
                .cursor
                .table_slice_by_index(2)
                .map_err(|_| CoreError::MalformedCobuild)?;
            update_cursor(&mut hasher, &type_cursor)?;
        }
        if mask_bit(&otx.base_output_masks, local_index * 4 + 3)? {
            update_len_prefixed(&mut hasher, output_data)?;
        }
    }

    update_count(&mut hasher, otx.base_cell_deps)?;
    update_len_prefixed(&mut hasher, &otx.base_cell_dep_masks)?;
    for local_index in 0..otx.base_cell_deps {
        if mask_bit(&otx.base_cell_dep_masks, local_index)? {
            let tx_index = checked_index(layout.base_cell_deps, local_index)?;
            update_count(&mut hasher, local_index)?;
            hasher.update(
                raw.cell_deps
                    .get(tx_index)
                    .ok_or(CoreError::MissingHashInput)?,
            );
        }
    }

    update_count(&mut hasher, otx.base_header_deps)?;
    update_len_prefixed(&mut hasher, &otx.base_header_dep_masks)?;
    for local_index in 0..otx.base_header_deps {
        if mask_bit(&otx.base_header_dep_masks, local_index)? {
            let tx_index = checked_index(layout.base_header_deps, local_index)?;
            update_count(&mut hasher, local_index)?;
            hasher.update(
                raw.header_deps
                    .get(tx_index)
                    .ok_or(CoreError::MissingHashInput)?,
            );
        }
    }

    hasher.finalize(&mut out);
    Ok(out)
}

pub fn otx_append_hash(
    otx: &OtxData,
    layout: &OtxLayout,
    raw: &RawTxParts,
    resolved_inputs: &[ResolvedInputHashPart],
    base_hash: [u8; 32],
) -> Result<[u8; 32], CoreError> {
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32)
        .personal(b"ckbcb_ota_core1\0")
        .build();

    hasher.update(&otx.message);
    hasher.update(&base_hash);
    update_count(&mut hasher, otx.append_input_cells)?;
    for local_index in 0..otx.append_input_cells {
        let tx_index = checked_index(layout.append_inputs, local_index)?;
        let input = raw
            .inputs
            .get(tx_index)
            .ok_or(CoreError::MissingHashInput)?;
        let resolved = resolved_inputs
            .get(tx_index)
            .ok_or(CoreError::MissingHashInput)?;
        update_count(&mut hasher, local_index)?;
        hasher.update(input);
        hasher.update(&resolved.output);
        update_len_prefixed(&mut hasher, &resolved.data)?;
    }

    update_count(&mut hasher, otx.append_output_cells)?;
    for local_index in 0..otx.append_output_cells {
        let tx_index = checked_index(layout.append_outputs, local_index)?;
        update_count(&mut hasher, local_index)?;
        hasher.update(
            raw.outputs
                .get(tx_index)
                .ok_or(CoreError::MissingHashInput)?,
        );
        update_len_prefixed(
            &mut hasher,
            raw.outputs_data
                .get(tx_index)
                .ok_or(CoreError::MissingHashInput)?,
        )?;
    }

    update_count(&mut hasher, otx.append_cell_deps)?;
    for local_index in 0..otx.append_cell_deps {
        let tx_index = checked_index(layout.append_cell_deps, local_index)?;
        update_count(&mut hasher, local_index)?;
        hasher.update(
            raw.cell_deps
                .get(tx_index)
                .ok_or(CoreError::MissingHashInput)?,
        );
    }

    update_count(&mut hasher, otx.append_header_deps)?;
    for local_index in 0..otx.append_header_deps {
        let tx_index = checked_index(layout.append_header_deps, local_index)?;
        update_count(&mut hasher, local_index)?;
        hasher.update(
            raw.header_deps
                .get(tx_index)
                .ok_or(CoreError::MissingHashInput)?,
        );
    }

    hasher.finalize(&mut out);
    Ok(out)
}

fn update_len_prefixed(hasher: &mut blake2b_ref::Blake2b, bytes: &[u8]) -> Result<(), CoreError> {
    hasher.update(&checked_len_prefix(bytes.len())?);
    hasher.update(bytes);
    Ok(())
}

fn update_count(hasher: &mut blake2b_ref::Blake2b, count: usize) -> Result<(), CoreError> {
    hasher.update(&checked_len_prefix(count)?);
    Ok(())
}

fn checked_index(range: Range, local_index: usize) -> Result<usize, CoreError> {
    if local_index >= range.count {
        return Err(CoreError::InvalidOtxLayout);
    }
    range
        .start
        .checked_add(local_index)
        .ok_or(CoreError::InvalidOtxLayout)
}

fn mask_bit(mask: &[u8], index: usize) -> Result<bool, CoreError> {
    let byte = mask.get(index / 8).ok_or(CoreError::InvalidOtxLayout)?;
    Ok(byte & (1 << (index % 8)) != 0)
}
