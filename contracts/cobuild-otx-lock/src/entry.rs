use crate::{
    args::parse_auth_args,
    chain::{load_current_script_args, load_prepared_context, load_script_hash},
    error::Error,
    errors::{map_core_error, map_verify_error},
    verify::{LockVerifier, local::LocalVerifier},
};

pub fn main() -> Result<(), Error> {
    let auth = parse_auth_args(&load_current_script_args()?)?;
    let current_script_hash = load_script_hash()?;
    let loaded = load_prepared_context()?;
    let signature_requests = loaded
        .prepared
        .context
        .lock_query(current_script_hash)
        .required_signatures(&loaded.source)
        .map_err(map_core_error)?;

    if signature_requests.is_empty() {
        return Err(Error::LockSemanticFailure);
    }

    let verifier = LocalVerifier;
    for request in &signature_requests {
        verifier
            .verify(&auth, &request.seal, &request.signing_message_hash)
            .map_err(map_verify_error)?;
    }

    Ok(())
}
