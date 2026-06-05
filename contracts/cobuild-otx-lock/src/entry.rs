use alloc::vec::Vec;
use ckb_std::{
    ckb_types::prelude::Unpack,
    high_level::{load_script, load_script_hash},
};
use cobuild_core::engine::CobuildContext;

use crate::{
    args::AuthContext,
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
    let plan = CobuildContext::from_syscalls()?.plan_lock_validation(current_script_hash)?;

    if plan.required_signatures.is_empty() {
        return Err(Error::LockSemanticFailure);
    }

    let verifier = LocalVerifier;
    for requirement in &plan.required_signatures {
        verifier.verify(&auth, &requirement.seal, &requirement.signing_message_hash)?;
    }

    Ok(())
}
