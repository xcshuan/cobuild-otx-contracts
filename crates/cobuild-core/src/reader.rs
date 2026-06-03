use alloc::{boxed::Box, vec, vec::Vec};
use core::cmp::min;

use cobuild_types::lazy_reader::support::{Cursor, Error as MoleculeError, Read};

use crate::error::CoreError;

pub struct OwnedReader {
    data: Vec<u8>,
}

impl OwnedReader {
    pub fn new(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
        }
    }
}

impl Read for OwnedReader {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, MoleculeError> {
        if offset >= self.data.len() {
            return Err(MoleculeError::OutOfBound(offset, self.data.len()));
        }

        let read_len = min(buf.len(), self.data.len() - offset);
        buf[..read_len].copy_from_slice(&self.data[offset..offset + read_len]);
        Ok(read_len)
    }
}

pub fn cursor_bytes(cursor: &Cursor) -> Result<Vec<u8>, CoreError> {
    cursor_bytes_with_error(cursor, CoreError::MalformedCobuild)
}

pub fn cursor_bytes_with_error(
    cursor: &Cursor,
    read_error: CoreError,
) -> Result<Vec<u8>, CoreError> {
    let mut bytes = vec![0; cursor.size];
    let read = cursor.read_at(&mut bytes).map_err(|_| read_error.clone())?;
    if read != bytes.len() {
        return Err(read_error);
    }
    Ok(bytes)
}

pub fn update_len_prefixed_cursor(
    hasher: &mut blake2b_ref::Blake2b,
    cursor: &Cursor,
    read_error: CoreError,
) -> Result<(), CoreError> {
    hasher.update(&crate::hash::checked_len_prefix(cursor.size)?);
    update_cursor_with_error(hasher, cursor, read_error)
}

pub fn update_cursor_with_error(
    hasher: &mut blake2b_ref::Blake2b,
    cursor: &Cursor,
    read_error: CoreError,
) -> Result<(), CoreError> {
    let mut offset = 0usize;
    let mut buf = [0u8; 256];

    while offset < cursor.size {
        let read_len = min(buf.len(), cursor.size - offset);
        let mut chunk = cursor.clone();
        chunk.add_offset(offset).map_err(|_| read_error.clone())?;
        chunk.size = read_len;

        let read = chunk
            .read_at(&mut buf[..read_len])
            .map_err(|_| read_error.clone())?;
        if read != read_len {
            return Err(read_error);
        }

        hasher.update(&buf[..read_len]);
        offset = offset
            .checked_add(read_len)
            .ok_or(CoreError::MalformedCobuild)?;
    }

    Ok(())
}

pub fn update_cursor(hasher: &mut blake2b_ref::Blake2b, cursor: &Cursor) -> Result<(), CoreError> {
    update_cursor_with_error(hasher, cursor, CoreError::MalformedCobuild)
}

pub fn cursor_from_slice(data: &[u8]) -> Cursor {
    let reader: Box<dyn Read> = Box::new(OwnedReader::new(data));
    Cursor::new(data.len(), reader)
}
