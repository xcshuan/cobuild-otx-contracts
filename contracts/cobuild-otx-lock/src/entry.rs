use alloc::vec::Vec;
use ckb_std::{
    ckb_types::prelude::Unpack,
    high_level::{load_script, load_script_hash},
};

use crate::{
    args::AuthContext,
    chain::load_prepared_context,
    error::Error,
    verify::{LockVerifier, local::LocalVerifier},
};

pub fn main() -> Result<(), Error> {
    let auth = {
        let script = load_script()?;
        let args: Vec<u8> = script.args().unpack();
        AuthContext::try_from(args.as_slice())?
    };

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
