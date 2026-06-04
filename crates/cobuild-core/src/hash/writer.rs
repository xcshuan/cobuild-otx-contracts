use blake2b_ref::Blake2b;
use cobuild_types::lazy_reader::support::Cursor;

use crate::{
    error::CoreError,
    reader::{update_cursor_with_error, update_len_prefixed_cursor},
    source::ClassifiedCursor,
};

pub(crate) fn write_count(hasher: &mut Blake2b, count: usize) -> Result<(), CoreError> {
    let count = u32::try_from(count).map_err(|_| CoreError::HashInputTooLarge)?;
    hasher.update(&count.to_le_bytes());
    Ok(())
}

pub(crate) fn write_cursor(
    hasher: &mut Blake2b,
    cursor: &ClassifiedCursor,
) -> Result<(), CoreError> {
    update_cursor_with_error(hasher, &cursor.cursor, cursor.read_error())
}

pub(crate) fn write_len_prefixed_cursor_with_error(
    hasher: &mut Blake2b,
    cursor: &Cursor,
    error: CoreError,
) -> Result<(), CoreError> {
    update_len_prefixed_cursor(hasher, cursor, error)
}

pub(crate) fn write_len_prefixed_classified_cursor(
    hasher: &mut Blake2b,
    cursor: &ClassifiedCursor,
) -> Result<(), CoreError> {
    update_len_prefixed_cursor(hasher, &cursor.cursor, cursor.read_error())
}
