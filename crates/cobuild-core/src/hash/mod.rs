mod writer;

use blake2b_ref::{Blake2b, Blake2bBuilder};
use cobuild_types::lazy_reader::{
    blockchain::{CellInput, CellOutput},
    support::Cursor,
};

use crate::{
    error::CoreError,
    layout::{OtxLayout, Range},
    syscalls,
    view::OtxView,
};

const TX_WITHOUT_MESSAGE_PERSONAL: &[u8; 16] = b"ckbcb_tnm_core1\0";
const TX_WITH_MESSAGE_PERSONAL: &[u8; 16] = b"ckbcb_twm_core1\0";
const OTX_BASE_PERSONAL: &[u8; 16] = b"ckbcb_otb_core1\0";
const OTX_APPEND_PERSONAL: &[u8; 16] = b"ckbcb_ota_core1\0";

pub(crate) fn tx_without_message_hash(
    reader: &syscalls::SyscallTxReader,
) -> Result<[u8; 32], CoreError> {
    tx_signing_hash(TX_WITHOUT_MESSAGE_PERSONAL, None, reader)
}

pub(crate) fn tx_with_message_hash(
    message: &Cursor,
    reader: &syscalls::SyscallTxReader,
) -> Result<[u8; 32], CoreError> {
    tx_signing_hash(TX_WITH_MESSAGE_PERSONAL, Some(message), reader)
}

fn tx_signing_hash(
    personalization: &[u8; 16],
    message: Option<&Cursor>,
    reader: &syscalls::SyscallTxReader,
) -> Result<[u8; 32], CoreError> {
    let mut hasher = new_signing_hasher(personalization);

    if let Some(message) = message {
        writer::write_cursor_with_error(&mut hasher, message, CoreError::MalformedCobuild)?;
    }
    hasher.update(&reader.tx_hash());
    let counts = reader.counts();
    for index in 0..counts.inputs {
        let output = reader.resolved_input_output_cursor(index)?;
        writer::write_cursor_with_error(&mut hasher, &output, CoreError::MissingHashInput)?;
        let data = reader.resolved_input_data_cursor(index)?;
        writer::write_len_prefixed_cursor_with_error(
            &mut hasher,
            &data,
            CoreError::MissingHashInput,
        )?;
    }
    for index in counts.inputs..counts.witnesses {
        let witness = reader.witness_cursor(index)?;
        writer::write_len_prefixed_cursor_with_error(
            &mut hasher,
            &witness,
            CoreError::MissingHashInput,
        )?;
    }

    Ok(finalize_hash(hasher))
}

pub(crate) fn checked_len_prefix(len: usize) -> Result<[u8; 4], CoreError> {
    let len = u32::try_from(len).map_err(|_| CoreError::HashInputTooLarge)?;
    Ok(len.to_le_bytes())
}

pub(crate) fn otx_base_hash(
    otx: &OtxView,
    layout: &OtxLayout,
    reader: &syscalls::SyscallTxReader,
) -> Result<[u8; 32], CoreError> {
    let mut hasher = new_signing_hasher(OTX_BASE_PERSONAL);

    writer::write_cursor_with_error(&mut hasher, &otx.message, CoreError::MalformedCobuild)?;
    hasher.update(&[otx.append_permissions]);
    write_otx_base_input_cells(&mut hasher, otx, layout, reader)?;
    write_otx_base_output_cells(&mut hasher, otx, layout, reader)?;
    write_otx_base_cell_deps(&mut hasher, otx, layout, reader)?;
    write_otx_base_header_deps(&mut hasher, otx, layout, reader)?;

    Ok(finalize_hash(hasher))
}

pub(crate) fn otx_append_hash(
    otx: &OtxView,
    layout: &OtxLayout,
    reader: &syscalls::SyscallTxReader,
    base_hash: [u8; 32],
) -> Result<[u8; 32], CoreError> {
    let mut hasher = new_signing_hasher(OTX_APPEND_PERSONAL);

    writer::write_cursor_with_error(&mut hasher, &otx.message, CoreError::MalformedCobuild)?;
    hasher.update(&base_hash);
    write_otx_append_input_cells(&mut hasher, otx, layout, reader)?;
    write_otx_append_output_cells(&mut hasher, otx, layout, reader)?;
    write_otx_append_cell_deps(&mut hasher, otx, layout, reader)?;
    write_otx_append_header_deps(&mut hasher, otx, layout, reader)?;

    Ok(finalize_hash(hasher))
}

fn new_signing_hasher(personalization: &[u8; 16]) -> Blake2b {
    Blake2bBuilder::new(32).personal(personalization).build()
}

fn finalize_hash(hasher: Blake2b) -> [u8; 32] {
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

fn write_default_out_point(hasher: &mut Blake2b) {
    hasher.update(&[0u8; 36]);
}

fn write_default_script(hasher: &mut Blake2b) {
    // Molecule encoding of packed::Script::default(); keep this crate ckb-std-free outside syscalls.
    hasher.update(&[
        53, 0, 0, 0, 16, 0, 0, 0, 48, 0, 0, 0, 49, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
}

fn write_default_script_opt(hasher: &mut Blake2b) {
    hasher.update(&[]);
}

fn write_default_cell_dep(hasher: &mut Blake2b) {
    hasher.update(&[0u8; 37]);
}

fn write_otx_base_input_cells(
    hasher: &mut Blake2b,
    otx: &OtxView,
    layout: &OtxLayout,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, otx.base_input_cells)?;
    writer::write_len_prefixed_bytes(hasher, otx.base_input_masks.bytes())?;
    for local_index in 0..otx.base_input_cells {
        let tx_index = checked_index(layout.base_inputs, local_index)?;
        let input = reader.raw_input_cursor(tx_index)?;
        let input_view = CellInput::from(input.clone());

        writer::write_count(hasher, local_index)?;
        if otx.includes_base_input_since(local_index)? {
            hasher.update(
                &input_view
                    .since()
                    .map_err(|_| CoreError::MissingHashInput)?
                    .to_le_bytes(),
            );
        } else {
            hasher.update(&0u64.to_le_bytes());
        }
        if otx.includes_base_input_previous_output(local_index)? {
            let previous_output = input_view
                .previous_output()
                .map_err(|_| CoreError::MissingHashInput)?;
            writer::write_cursor_with_error(
                hasher,
                &previous_output.cursor,
                CoreError::MissingHashInput,
            )?;
        } else {
            write_default_out_point(hasher);
        }
        let resolved_output = reader.resolved_input_output_cursor(tx_index)?;
        writer::write_cursor_with_error(hasher, &resolved_output, CoreError::MissingHashInput)?;
        let resolved_data = reader.resolved_input_data_cursor(tx_index)?;
        writer::write_len_prefixed_cursor_with_error(
            hasher,
            &resolved_data,
            CoreError::MissingHashInput,
        )?;
    }
    Ok(())
}

fn write_otx_base_output_cells(
    hasher: &mut Blake2b,
    otx: &OtxView,
    layout: &OtxLayout,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, otx.base_output_cells)?;
    writer::write_len_prefixed_bytes(hasher, otx.base_output_masks.bytes())?;
    for local_index in 0..otx.base_output_cells {
        let tx_index = checked_index(layout.base_outputs, local_index)?;
        let output = reader.raw_output_cursor(tx_index)?;
        let output_view = CellOutput::from(output.clone());

        writer::write_count(hasher, local_index)?;
        if otx.includes_base_output_capacity(local_index)? {
            hasher.update(
                &output_view
                    .capacity()
                    .map_err(|_| CoreError::MissingHashInput)?
                    .to_le_bytes(),
            );
        } else {
            hasher.update(&0u64.to_le_bytes());
        }
        if otx.includes_base_output_lock(local_index)? {
            let lock = output_view
                .lock()
                .map_err(|_| CoreError::MissingHashInput)?;
            writer::write_cursor_with_error(hasher, &lock.cursor, CoreError::MissingHashInput)?;
        } else {
            write_default_script(hasher);
        }
        if otx.includes_base_output_type(local_index)? {
            let type_cursor = output_view
                .cursor
                .table_slice_by_index(2)
                .map_err(|_| CoreError::MissingHashInput)?;
            writer::write_cursor_with_error(hasher, &type_cursor, CoreError::MissingHashInput)?;
        } else {
            write_default_script_opt(hasher);
        }
        if otx.includes_base_output_data(local_index)? {
            let output_data = reader.raw_output_data_cursor(tx_index)?;
            writer::write_len_prefixed_cursor_with_error(
                hasher,
                &output_data,
                CoreError::MissingHashInput,
            )?;
        } else {
            writer::write_len_prefixed_bytes(hasher, &[])?;
        }
    }
    Ok(())
}

fn write_otx_base_cell_deps(
    hasher: &mut Blake2b,
    otx: &OtxView,
    layout: &OtxLayout,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, otx.base_cell_deps)?;
    writer::write_len_prefixed_bytes(hasher, otx.base_cell_dep_masks.bytes())?;
    for local_index in 0..otx.base_cell_deps {
        writer::write_count(hasher, local_index)?;
        if otx.base_cell_dep_masks.get(local_index)? {
            let tx_index = checked_index(layout.base_cell_deps, local_index)?;
            let cell_dep = reader.raw_cell_dep_cursor(tx_index)?;
            writer::write_cursor_with_error(hasher, &cell_dep, CoreError::MissingHashInput)?;
        } else {
            write_default_cell_dep(hasher);
        }
    }
    Ok(())
}

fn write_otx_base_header_deps(
    hasher: &mut Blake2b,
    otx: &OtxView,
    layout: &OtxLayout,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, otx.base_header_deps)?;
    writer::write_len_prefixed_bytes(hasher, otx.base_header_dep_masks.bytes())?;
    for local_index in 0..otx.base_header_deps {
        writer::write_count(hasher, local_index)?;
        if otx.base_header_dep_masks.get(local_index)? {
            let tx_index = checked_index(layout.base_header_deps, local_index)?;
            hasher.update(&reader.raw_header_dep_hash(tx_index)?);
        } else {
            hasher.update(&[0u8; 32]);
        }
    }
    Ok(())
}

fn write_otx_append_input_cells(
    hasher: &mut Blake2b,
    otx: &OtxView,
    layout: &OtxLayout,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, otx.append_input_cells)?;
    for local_index in 0..otx.append_input_cells {
        let tx_index = checked_index(layout.append_inputs, local_index)?;
        let input = reader.raw_input_cursor(tx_index)?;
        writer::write_count(hasher, local_index)?;
        writer::write_cursor_with_error(hasher, &input, CoreError::MissingHashInput)?;
        let resolved_output = reader.resolved_input_output_cursor(tx_index)?;
        writer::write_cursor_with_error(hasher, &resolved_output, CoreError::MissingHashInput)?;
        let resolved_data = reader.resolved_input_data_cursor(tx_index)?;
        writer::write_len_prefixed_cursor_with_error(
            hasher,
            &resolved_data,
            CoreError::MissingHashInput,
        )?;
    }
    Ok(())
}

fn write_otx_append_output_cells(
    hasher: &mut Blake2b,
    otx: &OtxView,
    layout: &OtxLayout,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, otx.append_output_cells)?;
    for local_index in 0..otx.append_output_cells {
        let tx_index = checked_index(layout.append_outputs, local_index)?;
        writer::write_count(hasher, local_index)?;
        let output = reader.raw_output_cursor(tx_index)?;
        writer::write_cursor_with_error(hasher, &output, CoreError::MissingHashInput)?;
        let output_data = reader.raw_output_data_cursor(tx_index)?;
        writer::write_len_prefixed_cursor_with_error(
            hasher,
            &output_data,
            CoreError::MissingHashInput,
        )?;
    }
    Ok(())
}

fn write_otx_append_cell_deps(
    hasher: &mut Blake2b,
    otx: &OtxView,
    layout: &OtxLayout,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, otx.append_cell_deps)?;
    for local_index in 0..otx.append_cell_deps {
        let tx_index = checked_index(layout.append_cell_deps, local_index)?;
        writer::write_count(hasher, local_index)?;
        let cell_dep = reader.raw_cell_dep_cursor(tx_index)?;
        writer::write_cursor_with_error(hasher, &cell_dep, CoreError::MissingHashInput)?;
    }
    Ok(())
}

fn write_otx_append_header_deps(
    hasher: &mut Blake2b,
    otx: &OtxView,
    layout: &OtxLayout,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, otx.append_header_deps)?;
    for local_index in 0..otx.append_header_deps {
        let tx_index = checked_index(layout.append_header_deps, local_index)?;
        writer::write_count(hasher, local_index)?;
        hasher.update(&reader.raw_header_dep_hash(tx_index)?);
    }
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
