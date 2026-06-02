use ckb_std::error::SysError;
use cobuild_core::error::CoreError;

use crate::{error::Error, verify::VerifyError};

pub(crate) fn map_sys_error(err: SysError) -> Error {
    match err {
        SysError::ItemMissing => Error::LockSemanticFailure,
        _ => Error::SyscallFailure,
    }
}

pub(crate) fn map_core_error(err: CoreError) -> Error {
    match err {
        CoreError::MalformedCobuild
        | CoreError::InvalidOtxLayout
        | CoreError::InvalidMessageTarget
        | CoreError::DuplicateSighashAll
        | CoreError::MissingLockGroupCoverage
        | CoreError::MissingSealPair
        | CoreError::DuplicateSealPair
        | CoreError::InvalidSealScope => Error::MalformedCobuild,
        CoreError::InvalidContextInput
        | CoreError::MissingHashInput
        | CoreError::HashInputTooLarge => Error::InternalFailure,
    }
}

pub(crate) fn map_verify_error(_err: VerifyError) -> Error {
    Error::VerifyFailure
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_protocol_errors_map_to_malformed_cobuild() {
        for err in [
            CoreError::MalformedCobuild,
            CoreError::InvalidOtxLayout,
            CoreError::InvalidMessageTarget,
            CoreError::DuplicateSighashAll,
            CoreError::MissingLockGroupCoverage,
            CoreError::MissingSealPair,
            CoreError::DuplicateSealPair,
            CoreError::InvalidSealScope,
        ] {
            assert_eq!(map_core_error(err), Error::MalformedCobuild);
        }
    }

    #[test]
    fn internal_input_errors_map_to_internal_failure() {
        for err in [
            CoreError::InvalidContextInput,
            CoreError::MissingHashInput,
            CoreError::HashInputTooLarge,
        ] {
            assert_eq!(map_core_error(err), Error::InternalFailure);
        }
    }
}
