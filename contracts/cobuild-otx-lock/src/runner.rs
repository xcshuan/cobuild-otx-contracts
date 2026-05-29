use alloc::{vec, vec::Vec};

use ckb_std::{
    ckb_constants::{CellField, Source},
    error::SysError,
    syscalls,
};
use cobuild_core::{
    error::CoreError,
    hash::ResolvedInputHashPart,
    loader::{
        PreparedContextInput, parse_transaction_info, prepare_context, script_args_from_slice,
    },
};

use crate::{
    args::parse_auth_args,
    error::Error,
    verify::{LockVerifier, VerifyError},
};

pub fn run(verifier: &impl LockVerifier) -> Result<(), Error> {
    let auth = parse_auth_args(&current_script_args()?)?;
    let current_script_hash = load_script_hash()?;
    let prepared = load_prepared_context()?;
    let tx_tasks = prepared
        .context
        .lock_query(current_script_hash)
        .tx_tasks(&prepared.hash_parts)
        .map_err(map_core_error)?;
    let otx_tasks = prepared
        .context
        .lock_query(current_script_hash)
        .otx_tasks(&prepared.hash_parts)
        .map_err(map_core_error)?;

    if tx_tasks.is_empty() && otx_tasks.is_empty() {
        return Err(Error::LockSemanticFailure);
    }

    for task in &tx_tasks {
        verifier
            .verify(&auth, &task.seal, &task.signing_message_hash)
            .map_err(map_verify_error)?;
    }
    for task in &otx_tasks {
        verifier
            .verify(&auth, &task.seal, &task.signing_message_hash)
            .map_err(map_verify_error)?;
    }

    Ok(())
}

fn current_script_args() -> Result<Vec<u8>, Error> {
    script_args_from_slice(&load_script()?).map_err(map_core_error)
}

fn load_prepared_context() -> Result<cobuild_core::context::PreparedContext, Error> {
    let info = parse_transaction_info(&load_transaction()?).map_err(map_core_error)?;
    let input_count = info.input_count;
    let output_count = info.output_count;
    let witnesses = info.witnesses;
    let trailing_witnesses = witnesses.iter().skip(input_count).cloned().collect();

    prepare_context(PreparedContextInput {
        witnesses,
        input_count,
        output_count,
        cell_dep_count: info.cell_dep_count,
        header_dep_count: info.header_dep_count,
        input_locks: load_input_lock_hashes(input_count)?,
        input_types: load_type_hashes(input_count, Source::Input)?,
        output_types: load_type_hashes(output_count, Source::Output)?,
        tx_hash: load_tx_hash()?,
        resolved_inputs: load_resolved_inputs(input_count)?,
        trailing_witnesses,
        raw_parts: info.raw_parts,
    })
    .map_err(map_core_error)
}

fn load_input_lock_hashes(input_count: usize) -> Result<Vec<[u8; 32]>, Error> {
    let mut hashes = Vec::with_capacity(input_count);
    for index in 0..input_count {
        hashes.push(load_cell_field_hash(
            index,
            Source::Input,
            CellField::LockHash,
        )?);
    }
    Ok(hashes)
}

fn load_type_hashes(count: usize, source: Source) -> Result<Vec<Option<[u8; 32]>>, Error> {
    let mut hashes = Vec::with_capacity(count);
    for index in 0..count {
        hashes.push(
            match load_cell_field_hash(index, source, CellField::TypeHash) {
                Ok(hash) => Some(hash),
                Err(Error::LockSemanticFailure) => None,
                Err(err) => return Err(err),
            },
        );
    }
    Ok(hashes)
}

fn load_resolved_inputs(input_count: usize) -> Result<Vec<ResolvedInputHashPart>, Error> {
    let mut inputs = Vec::with_capacity(input_count);
    for index in 0..input_count {
        inputs.push(ResolvedInputHashPart {
            output: load_cell(index, Source::Input)?,
            data: load_cell_data(index, Source::Input)?,
        });
    }
    Ok(inputs)
}

fn load_tx_hash() -> Result<[u8; 32], Error> {
    let mut hash = [0u8; 32];
    syscalls::load_tx_hash(&mut hash, 0).map_err(map_sys_error)?;
    Ok(hash)
}

fn load_script_hash() -> Result<[u8; 32], Error> {
    let mut hash = [0u8; 32];
    syscalls::load_script_hash(&mut hash, 0).map_err(map_sys_error)?;
    Ok(hash)
}

fn load_script() -> Result<Vec<u8>, Error> {
    load_data(|buf, offset| syscalls::load_script(buf, offset))
}

fn load_transaction() -> Result<Vec<u8>, Error> {
    load_data(|buf, offset| syscalls::load_transaction(buf, offset))
}

fn load_cell(index: usize, source: Source) -> Result<Vec<u8>, Error> {
    load_data(|buf, offset| syscalls::load_cell(buf, offset, index, source))
}

fn load_cell_data(index: usize, source: Source) -> Result<Vec<u8>, Error> {
    load_data(|buf, offset| syscalls::load_cell_data(buf, offset, index, source))
}

fn load_cell_field_hash(index: usize, source: Source, field: CellField) -> Result<[u8; 32], Error> {
    let mut hash = [0u8; 32];
    syscalls::load_cell_by_field(&mut hash, 0, index, source, field).map_err(map_sys_error)?;
    Ok(hash)
}

fn load_data(
    syscall: impl Fn(&mut [u8], usize) -> Result<usize, SysError>,
) -> Result<Vec<u8>, Error> {
    let mut buf = [0u8; 256];
    match syscall(&mut buf, 0) {
        Ok(len) => Ok(buf[..len].to_vec()),
        Err(SysError::LengthNotEnough(actual_size)) => {
            let mut data = vec![0; actual_size];
            let loaded_len = buf.len();
            data[..loaded_len].copy_from_slice(&buf);
            syscall(&mut data[loaded_len..], loaded_len).map_err(map_sys_error)?;
            Ok(data)
        }
        Err(err) => Err(map_sys_error(err)),
    }
}

fn map_sys_error(err: SysError) -> Error {
    match err {
        SysError::ItemMissing => Error::LockSemanticFailure,
        _ => Error::SyscallFailure,
    }
}

fn map_core_error(err: CoreError) -> Error {
    match err {
        CoreError::MalformedCobuild => Error::MalformedCobuild,
        CoreError::InvalidLayout
        | CoreError::InvalidMessageTarget
        | CoreError::MissingSealPair
        | CoreError::DuplicateSealPair => Error::LockSemanticFailure,
        CoreError::MissingHashParts => Error::InternalFailure,
    }
}

fn map_verify_error(_err: VerifyError) -> Error {
    Error::VerifyFailure
}
