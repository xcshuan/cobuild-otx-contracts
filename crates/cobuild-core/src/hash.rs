use blake2b_ref::Blake2bBuilder;
use cobuild_types::lazy_reader::{
    blockchain::{CellInput, CellOutput},
    support::Cursor,
};

use crate::{
    error::CoreError,
    layout::{OtxLayout, Range},
    reader::{update_cursor_with_error, update_len_prefixed_cursor},
    source::SigningDataSource,
    view::{MaskView, OtxView},
};

pub fn tx_without_message_hash<S: SigningDataSource>(source: &S) -> Result<[u8; 32], CoreError> {
    tx_signing_hash(b"ckbcb_tnm_core1\0", None, source)
}

pub fn tx_with_message_hash<S: SigningDataSource>(
    message: &Cursor,
    source: &S,
) -> Result<[u8; 32], CoreError> {
    tx_signing_hash(b"ckbcb_twm_core1\0", Some(message), source)
}

fn tx_signing_hash<S: SigningDataSource>(
    personalization: &[u8; 16],
    message: Option<&Cursor>,
    source: &S,
) -> Result<[u8; 32], CoreError> {
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32).personal(personalization).build();

    if let Some(message) = message {
        update_cursor_with_error(&mut hasher, message, CoreError::MalformedCobuild)?;
    }
    hasher.update(&source.tx_hash()?);
    let input_count = source.input_count()?;
    for index in 0..input_count {
        let output = source.resolved_input_output_cursor(index)?;
        update_cursor_with_error(&mut hasher, &output.cursor, output.read_error())?;
        let data = source.resolved_input_data_cursor(index)?;
        update_len_prefixed_cursor(&mut hasher, &data.cursor, data.read_error())?;
    }
    for index in input_count..source.witness_count()? {
        let witness = source.witness_cursor(index)?;
        update_len_prefixed_cursor(&mut hasher, &witness.cursor, witness.read_error())?;
    }
    hasher.finalize(&mut out);

    Ok(out)
}

pub fn checked_len_prefix(len: usize) -> Result<[u8; 4], CoreError> {
    let len = u32::try_from(len).map_err(|_| CoreError::HashInputTooLarge)?;
    Ok(len.to_le_bytes())
}

pub fn otx_base_hash<S: SigningDataSource>(
    otx: &OtxView,
    layout: &OtxLayout,
    source: &S,
) -> Result<[u8; 32], CoreError> {
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32)
        .personal(b"ckbcb_otb_core1\0")
        .build();

    update_cursor_with_error(&mut hasher, &otx.message, CoreError::MalformedCobuild)?;
    hasher.update(&[otx.append_permissions]);
    update_count(&mut hasher, otx.base_input_cells)?;
    update_len_prefixed_cursor(
        &mut hasher,
        otx.base_input_masks.cursor(),
        CoreError::MalformedCobuild,
    )?;
    for local_index in 0..otx.base_input_cells {
        let tx_index = checked_index(layout.base_inputs, local_index)?;
        let input = source.raw_input_cursor(tx_index)?;
        let input_view = CellInput::from(input.cursor.clone());

        update_count(&mut hasher, local_index)?;
        if mask_bit(&otx.base_input_masks, local_index * 2)? {
            hasher.update(
                &input_view
                    .since()
                    .map_err(|_| input.read_error())?
                    .to_le_bytes(),
            );
        }
        if mask_bit(&otx.base_input_masks, local_index * 2 + 1)? {
            let previous_output = input_view
                .previous_output()
                .map_err(|_| input.read_error())?;
            update_cursor_with_error(&mut hasher, &previous_output.cursor, input.read_error())?;
        }
        let resolved_output = source.resolved_input_output_cursor(tx_index)?;
        update_cursor_with_error(
            &mut hasher,
            &resolved_output.cursor,
            resolved_output.read_error(),
        )?;
        let resolved_data = source.resolved_input_data_cursor(tx_index)?;
        update_len_prefixed_cursor(
            &mut hasher,
            &resolved_data.cursor,
            resolved_data.read_error(),
        )?;
    }

    update_count(&mut hasher, otx.base_output_cells)?;
    update_len_prefixed_cursor(
        &mut hasher,
        otx.base_output_masks.cursor(),
        CoreError::MalformedCobuild,
    )?;
    for local_index in 0..otx.base_output_cells {
        let tx_index = checked_index(layout.base_outputs, local_index)?;
        let output = source.raw_output_cursor(tx_index)?;
        let output_view = CellOutput::from(output.cursor.clone());

        update_count(&mut hasher, local_index)?;
        if mask_bit(&otx.base_output_masks, local_index * 4)? {
            hasher.update(
                &output_view
                    .capacity()
                    .map_err(|_| output.read_error())?
                    .to_le_bytes(),
            );
        }
        if mask_bit(&otx.base_output_masks, local_index * 4 + 1)? {
            let lock = output_view.lock().map_err(|_| output.read_error())?;
            update_cursor_with_error(&mut hasher, &lock.cursor, output.read_error())?;
        }
        if mask_bit(&otx.base_output_masks, local_index * 4 + 2)? {
            let type_cursor = output_view
                .cursor
                .table_slice_by_index(2)
                .map_err(|_| output.read_error())?;
            update_cursor_with_error(&mut hasher, &type_cursor, output.read_error())?;
        }
        if mask_bit(&otx.base_output_masks, local_index * 4 + 3)? {
            let output_data = source.raw_output_data_cursor(tx_index)?;
            update_len_prefixed_cursor(&mut hasher, &output_data.cursor, output_data.read_error())?;
        }
    }

    update_count(&mut hasher, otx.base_cell_deps)?;
    update_len_prefixed_cursor(
        &mut hasher,
        otx.base_cell_dep_masks.cursor(),
        CoreError::MalformedCobuild,
    )?;
    for local_index in 0..otx.base_cell_deps {
        if mask_bit(&otx.base_cell_dep_masks, local_index)? {
            let tx_index = checked_index(layout.base_cell_deps, local_index)?;
            let cell_dep = source.raw_cell_dep_cursor(tx_index)?;
            update_count(&mut hasher, local_index)?;
            update_cursor_with_error(&mut hasher, &cell_dep.cursor, cell_dep.read_error())?;
        }
    }

    update_count(&mut hasher, otx.base_header_deps)?;
    update_len_prefixed_cursor(
        &mut hasher,
        otx.base_header_dep_masks.cursor(),
        CoreError::MalformedCobuild,
    )?;
    for local_index in 0..otx.base_header_deps {
        if mask_bit(&otx.base_header_dep_masks, local_index)? {
            let tx_index = checked_index(layout.base_header_deps, local_index)?;
            update_count(&mut hasher, local_index)?;
            hasher.update(&source.raw_header_dep_hash(tx_index)?);
        }
    }

    hasher.finalize(&mut out);
    Ok(out)
}

pub fn otx_append_hash<S: SigningDataSource>(
    otx: &OtxView,
    layout: &OtxLayout,
    source: &S,
    base_hash: [u8; 32],
) -> Result<[u8; 32], CoreError> {
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32)
        .personal(b"ckbcb_ota_core1\0")
        .build();

    update_cursor_with_error(&mut hasher, &otx.message, CoreError::MalformedCobuild)?;
    hasher.update(&base_hash);
    update_count(&mut hasher, otx.append_input_cells)?;
    for local_index in 0..otx.append_input_cells {
        let tx_index = checked_index(layout.append_inputs, local_index)?;
        let input = source.raw_input_cursor(tx_index)?;
        update_count(&mut hasher, local_index)?;
        update_cursor_with_error(&mut hasher, &input.cursor, input.read_error())?;
        let resolved_output = source.resolved_input_output_cursor(tx_index)?;
        update_cursor_with_error(
            &mut hasher,
            &resolved_output.cursor,
            resolved_output.read_error(),
        )?;
        let resolved_data = source.resolved_input_data_cursor(tx_index)?;
        update_len_prefixed_cursor(
            &mut hasher,
            &resolved_data.cursor,
            resolved_data.read_error(),
        )?;
    }

    update_count(&mut hasher, otx.append_output_cells)?;
    for local_index in 0..otx.append_output_cells {
        let tx_index = checked_index(layout.append_outputs, local_index)?;
        update_count(&mut hasher, local_index)?;
        let output = source.raw_output_cursor(tx_index)?;
        update_cursor_with_error(&mut hasher, &output.cursor, output.read_error())?;
        let output_data = source.raw_output_data_cursor(tx_index)?;
        update_len_prefixed_cursor(&mut hasher, &output_data.cursor, output_data.read_error())?;
    }

    update_count(&mut hasher, otx.append_cell_deps)?;
    for local_index in 0..otx.append_cell_deps {
        let tx_index = checked_index(layout.append_cell_deps, local_index)?;
        update_count(&mut hasher, local_index)?;
        let cell_dep = source.raw_cell_dep_cursor(tx_index)?;
        update_cursor_with_error(&mut hasher, &cell_dep.cursor, cell_dep.read_error())?;
    }

    update_count(&mut hasher, otx.append_header_deps)?;
    for local_index in 0..otx.append_header_deps {
        let tx_index = checked_index(layout.append_header_deps, local_index)?;
        update_count(&mut hasher, local_index)?;
        hasher.update(&source.raw_header_dep_hash(tx_index)?);
    }

    hasher.finalize(&mut out);
    Ok(out)
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

fn mask_bit(mask: &MaskView, index: usize) -> Result<bool, CoreError> {
    mask.bit(index)
}
