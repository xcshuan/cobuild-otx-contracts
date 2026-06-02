use crate::{
    args::parse_auth_args,
    error::Error,
    errors::{map_core_error, map_verify_error},
    loader::{load_current_script_args, load_prepared_context, load_script_hash},
    verify::{LockVerifier, local::LocalVerifier},
};

pub fn main() -> Result<(), Error> {
    let auth = parse_auth_args(&load_current_script_args()?)?;
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

    let verifier = LocalVerifier;
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
