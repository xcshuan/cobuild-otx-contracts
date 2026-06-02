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
        | CoreError::InvalidLayout
        | CoreError::InvalidMessageTarget
        | CoreError::MissingSealPair
        | CoreError::DuplicateSealPair => Error::MalformedCobuild,
        CoreError::MissingHashParts => Error::InternalFailure,
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
            CoreError::InvalidLayout,
            CoreError::InvalidMessageTarget,
            CoreError::MissingSealPair,
            CoreError::DuplicateSealPair,
        ] {
            assert_eq!(map_core_error(err), Error::MalformedCobuild);
        }
    }

    #[test]
    fn missing_hash_parts_maps_to_internal_failure() {
        assert_eq!(
            map_core_error(CoreError::MissingHashParts),
            Error::InternalFailure
        );
    }
}
