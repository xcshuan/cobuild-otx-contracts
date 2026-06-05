mod writer;

use blake2b_ref::Blake2bBuilder;
use cobuild_types::lazy_reader::{
    blockchain::{CellInput, CellOutput},
    support::Cursor,
};

use crate::{
    error::CoreError,
    layout::{OtxLayout, Range},
    syscalls,
    view::{MaskView, OtxView},
};

pub(crate) fn tx_without_message_hash(
    counts_cache: &syscalls::TxCountsCache,
) -> Result<[u8; 32], CoreError> {
    tx_signing_hash(b"ckbcb_tnm_core1\0", None, counts_cache)
}

pub(crate) fn tx_with_message_hash(
    message: &Cursor,
    counts_cache: &syscalls::TxCountsCache,
) -> Result<[u8; 32], CoreError> {
    tx_signing_hash(b"ckbcb_twm_core1\0", Some(message), counts_cache)
}

fn tx_signing_hash(
    personalization: &[u8; 16],
    message: Option<&Cursor>,
    counts_cache: &syscalls::TxCountsCache,
) -> Result<[u8; 32], CoreError> {
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32).personal(personalization).build();

    if let Some(message) = message {
        writer::write_cursor_with_error(&mut hasher, message, CoreError::MalformedCobuild)?;
    }
    hasher.update(&syscalls::tx_hash()?);
    let counts = syscalls::counts(counts_cache)?;
    for index in 0..counts.inputs {
        let output = syscalls::resolved_input_output_cursor(index)?;
        writer::write_cursor_with_error(&mut hasher, &output, CoreError::MissingHashInput)?;
        let data = syscalls::resolved_input_data_cursor(index)?;
        writer::write_len_prefixed_cursor_with_error(
            &mut hasher,
            &data,
            CoreError::MissingHashInput,
        )?;
    }
    for index in counts.inputs..counts.witnesses {
        let witness = syscalls::witness_cursor(index)?;
        writer::write_len_prefixed_cursor_with_error(
            &mut hasher,
            &witness,
            CoreError::MissingHashInput,
        )?;
    }
    hasher.finalize(&mut out);

    Ok(out)
}

pub(crate) fn checked_len_prefix(len: usize) -> Result<[u8; 4], CoreError> {
    let len = u32::try_from(len).map_err(|_| CoreError::HashInputTooLarge)?;
    Ok(len.to_le_bytes())
}

pub(crate) fn otx_base_hash(
    otx: &OtxView,
    layout: &OtxLayout,
    _counts_cache: &syscalls::TxCountsCache,
) -> Result<[u8; 32], CoreError> {
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32)
        .personal(b"ckbcb_otb_core1\0")
        .build();

    writer::write_cursor_with_error(&mut hasher, &otx.message, CoreError::MalformedCobuild)?;
    hasher.update(&[otx.append_permissions]);
    writer::write_count(&mut hasher, otx.base_input_cells)?;
    writer::write_len_prefixed_cursor_with_error(
        &mut hasher,
        otx.base_input_masks.cursor(),
        CoreError::MalformedCobuild,
    )?;
    for local_index in 0..otx.base_input_cells {
        let tx_index = checked_index(layout.base_inputs, local_index)?;
        let input = syscalls::raw_input_cursor(tx_index)?;
        let input_view = CellInput::from(input.clone());

        writer::write_count(&mut hasher, local_index)?;
        if mask_bit(&otx.base_input_masks, local_index * 2)? {
            hasher.update(
                &input_view
                    .since()
                    .map_err(|_| CoreError::MissingHashInput)?
                    .to_le_bytes(),
            );
        }
        if mask_bit(&otx.base_input_masks, local_index * 2 + 1)? {
            let previous_output = input_view
                .previous_output()
                .map_err(|_| CoreError::MissingHashInput)?;
            writer::write_cursor_with_error(
                &mut hasher,
                &previous_output.cursor,
                CoreError::MissingHashInput,
            )?;
        }
        let resolved_output = syscalls::resolved_input_output_cursor(tx_index)?;
        writer::write_cursor_with_error(
            &mut hasher,
            &resolved_output,
            CoreError::MissingHashInput,
        )?;
        let resolved_data = syscalls::resolved_input_data_cursor(tx_index)?;
        writer::write_len_prefixed_cursor_with_error(
            &mut hasher,
            &resolved_data,
            CoreError::MissingHashInput,
        )?;
    }

    writer::write_count(&mut hasher, otx.base_output_cells)?;
    writer::write_len_prefixed_cursor_with_error(
        &mut hasher,
        otx.base_output_masks.cursor(),
        CoreError::MalformedCobuild,
    )?;
    for local_index in 0..otx.base_output_cells {
        let tx_index = checked_index(layout.base_outputs, local_index)?;
        let output = syscalls::raw_output_cursor(tx_index)?;
        let output_view = CellOutput::from(output.clone());

        writer::write_count(&mut hasher, local_index)?;
        if mask_bit(&otx.base_output_masks, local_index * 4)? {
            hasher.update(
                &output_view
                    .capacity()
                    .map_err(|_| CoreError::MissingHashInput)?
                    .to_le_bytes(),
            );
        }
        if mask_bit(&otx.base_output_masks, local_index * 4 + 1)? {
            let lock = output_view
                .lock()
                .map_err(|_| CoreError::MissingHashInput)?;
            writer::write_cursor_with_error(
                &mut hasher,
                &lock.cursor,
                CoreError::MissingHashInput,
            )?;
        }
        if mask_bit(&otx.base_output_masks, local_index * 4 + 2)? {
            let type_cursor = output_view
                .cursor
                .table_slice_by_index(2)
                .map_err(|_| CoreError::MissingHashInput)?;
            writer::write_cursor_with_error(
                &mut hasher,
                &type_cursor,
                CoreError::MissingHashInput,
            )?;
        }
        if mask_bit(&otx.base_output_masks, local_index * 4 + 3)? {
            let output_data = syscalls::raw_output_data_cursor(tx_index)?;
            writer::write_len_prefixed_cursor_with_error(
                &mut hasher,
                &output_data,
                CoreError::MissingHashInput,
            )?;
        }
    }

    writer::write_count(&mut hasher, otx.base_cell_deps)?;
    writer::write_len_prefixed_cursor_with_error(
        &mut hasher,
        otx.base_cell_dep_masks.cursor(),
        CoreError::MalformedCobuild,
    )?;
    for local_index in 0..otx.base_cell_deps {
        if mask_bit(&otx.base_cell_dep_masks, local_index)? {
            let tx_index = checked_index(layout.base_cell_deps, local_index)?;
            let cell_dep = syscalls::raw_cell_dep_cursor(tx_index)?;
            writer::write_count(&mut hasher, local_index)?;
            writer::write_cursor_with_error(&mut hasher, &cell_dep, CoreError::MissingHashInput)?;
        }
    }

    writer::write_count(&mut hasher, otx.base_header_deps)?;
    writer::write_len_prefixed_cursor_with_error(
        &mut hasher,
        otx.base_header_dep_masks.cursor(),
        CoreError::MalformedCobuild,
    )?;
    for local_index in 0..otx.base_header_deps {
        if mask_bit(&otx.base_header_dep_masks, local_index)? {
            let tx_index = checked_index(layout.base_header_deps, local_index)?;
            writer::write_count(&mut hasher, local_index)?;
            hasher.update(&syscalls::raw_header_dep_hash(tx_index)?);
        }
    }

    hasher.finalize(&mut out);
    Ok(out)
}

pub(crate) fn otx_append_hash(
    otx: &OtxView,
    layout: &OtxLayout,
    _counts_cache: &syscalls::TxCountsCache,
    base_hash: [u8; 32],
) -> Result<[u8; 32], CoreError> {
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32)
        .personal(b"ckbcb_ota_core1\0")
        .build();

    writer::write_cursor_with_error(&mut hasher, &otx.message, CoreError::MalformedCobuild)?;
    hasher.update(&base_hash);
    writer::write_count(&mut hasher, otx.append_input_cells)?;
    for local_index in 0..otx.append_input_cells {
        let tx_index = checked_index(layout.append_inputs, local_index)?;
        let input = syscalls::raw_input_cursor(tx_index)?;
        writer::write_count(&mut hasher, local_index)?;
        writer::write_cursor_with_error(&mut hasher, &input, CoreError::MissingHashInput)?;
        let resolved_output = syscalls::resolved_input_output_cursor(tx_index)?;
        writer::write_cursor_with_error(
            &mut hasher,
            &resolved_output,
            CoreError::MissingHashInput,
        )?;
        let resolved_data = syscalls::resolved_input_data_cursor(tx_index)?;
        writer::write_len_prefixed_cursor_with_error(
            &mut hasher,
            &resolved_data,
            CoreError::MissingHashInput,
        )?;
    }

    writer::write_count(&mut hasher, otx.append_output_cells)?;
    for local_index in 0..otx.append_output_cells {
        let tx_index = checked_index(layout.append_outputs, local_index)?;
        writer::write_count(&mut hasher, local_index)?;
        let output = syscalls::raw_output_cursor(tx_index)?;
        writer::write_cursor_with_error(&mut hasher, &output, CoreError::MissingHashInput)?;
        let output_data = syscalls::raw_output_data_cursor(tx_index)?;
        writer::write_len_prefixed_cursor_with_error(
            &mut hasher,
            &output_data,
            CoreError::MissingHashInput,
        )?;
    }

    writer::write_count(&mut hasher, otx.append_cell_deps)?;
    for local_index in 0..otx.append_cell_deps {
        let tx_index = checked_index(layout.append_cell_deps, local_index)?;
        writer::write_count(&mut hasher, local_index)?;
        let cell_dep = syscalls::raw_cell_dep_cursor(tx_index)?;
        writer::write_cursor_with_error(&mut hasher, &cell_dep, CoreError::MissingHashInput)?;
    }

    writer::write_count(&mut hasher, otx.append_header_deps)?;
    for local_index in 0..otx.append_header_deps {
        let tx_index = checked_index(layout.append_header_deps, local_index)?;
        writer::write_count(&mut hasher, local_index)?;
        hasher.update(&syscalls::raw_header_dep_hash(tx_index)?);
    }

    hasher.finalize(&mut out);
    Ok(out)
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
