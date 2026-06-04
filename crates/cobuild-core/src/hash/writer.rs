use blake2b_ref::Blake2b;
use cobuild_types::lazy_reader::support::Cursor;

use crate::{error::CoreError, hash::checked_len_prefix, reader::update_cursor_with_error};

pub(crate) fn write_count(hasher: &mut Blake2b, count: usize) -> Result<(), CoreError> {
    hasher.update(&checked_len_prefix(count)?);
    Ok(())
}

pub(crate) fn write_cursor_with_error(
    hasher: &mut Blake2b,
    cursor: &Cursor,
    error: CoreError,
) -> Result<(), CoreError> {
    update_cursor_with_error(hasher, cursor, error)
}

pub(crate) fn write_len_prefixed_cursor_with_error(
    hasher: &mut Blake2b,
    cursor: &Cursor,
    error: CoreError,
) -> Result<(), CoreError> {
    hasher.update(&checked_len_prefix(cursor.size)?);
    update_cursor_with_error(hasher, cursor, error)
}
