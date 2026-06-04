use alloc::vec::Vec;
use core::convert::TryInto;

use cobuild_types::lazy_reader::blockchain::Script;

use crate::{error::CoreError, reader::cursor_from_slice};

pub fn script_args_from_slice(data: &[u8]) -> Result<Vec<u8>, CoreError> {
    let script = Script::from(cursor_from_slice(data));
    script
        .verify(false)
        .map_err(|_| CoreError::InvalidOtxLayout)?;
    script
        .args()
        .and_then(TryInto::try_into)
        .map_err(|_| CoreError::MalformedCobuild)
}
