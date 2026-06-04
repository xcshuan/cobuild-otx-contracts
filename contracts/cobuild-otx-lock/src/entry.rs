use crate::{
    args::parse_auth_args,
    chain::{load_current_script_args, load_prepared_context, load_script_hash},
    error::Error,
    verify::{LockVerifier, local::LocalVerifier},
};

pub fn main() -> Result<(), Error> {
    let auth = parse_auth_args(&load_current_script_args()?)?;
    let current_script_hash = load_script_hash()?;
    let loaded = load_prepared_context()?;
    let plan = loaded
        .prepared
        .plan_lock_validation(current_script_hash, &loaded.source)?;

    if plan.required_signatures.is_empty() {
        return Err(Error::LockSemanticFailure);
    }

    let verifier = LocalVerifier;
    for requirement in &plan.required_signatures {
        verifier.verify(&auth, &requirement.seal, &requirement.signing_message_hash)?;
    }

    Ok(())
}
