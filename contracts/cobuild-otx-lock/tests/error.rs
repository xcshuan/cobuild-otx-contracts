use ckb_std::error::SysError;
use cobuild_core::error::CoreError;
use cobuild_otx_lock::error::Error;

#[test]
fn error_codes_are_grouped_by_category() {
    assert_eq!(Error::SysIndexOutOfBound.code(), 1);
    assert_eq!(Error::SysItemMissing.code(), 2);
    assert_eq!(Error::SysLengthNotEnough.code(), 3);
    assert_eq!(Error::SysEncoding.code(), 4);
    assert_eq!(Error::SysWaitFailure.code(), 5);
    assert_eq!(Error::SysInvalidFd.code(), 6);
    assert_eq!(Error::SysOtherEndClosed.code(), 7);
    assert_eq!(Error::SysMaxVmsSpawned.code(), 8);
    assert_eq!(Error::SysMaxFdsCreated.code(), 9);
    assert_eq!(Error::SyscallUnknown.code(), 10);

    assert_eq!(Error::InvalidArgs.code(), 20);
    assert_eq!(Error::MalformedCobuild.code(), 30);
    assert_eq!(Error::InvalidOtxLayout.code(), 31);
    assert_eq!(Error::InvalidMessageTarget.code(), 32);
    assert_eq!(Error::DuplicateSighashAll.code(), 33);
    assert_eq!(Error::MissingLockGroupCoverage.code(), 34);
    assert_eq!(Error::MissingSealPair.code(), 35);
    assert_eq!(Error::DuplicateSealPair.code(), 36);
    assert_eq!(Error::InvalidSealScope.code(), 37);
    assert_eq!(Error::DuplicateMatchingAction.code(), 38);
    assert_eq!(Error::InvalidLockGroupWitness.code(), 39);

    assert_eq!(Error::LockSemanticFailure.code(), 40);
    assert_eq!(Error::VerifyFailure.code(), 50);
    assert_eq!(Error::InternalFailure.code(), 60);
    assert_eq!(Error::InvalidContextInput.code(), 61);
    assert_eq!(Error::MissingHashInput.code(), 62);
    assert_eq!(Error::HashInputTooLarge.code(), 63);
}

#[test]
fn errors_convert_to_stable_exit_codes() {
    assert_eq!(i8::from(Error::InvalidArgs), 20);
    assert_eq!(i8::from(Error::MalformedCobuild), 30);
    assert_eq!(i8::from(Error::InvalidOtxLayout), 31);
    assert_eq!(i8::from(Error::LockSemanticFailure), 40);
    assert_eq!(i8::from(Error::VerifyFailure), 50);
    assert_eq!(i8::from(Error::InternalFailure), 60);
    assert_eq!(i8::from(Error::MissingHashInput), 62);
}

#[test]
fn sys_errors_map_to_contract_errors() {
    assert_eq!(Error::from(SysError::ItemMissing), Error::SysItemMissing);
    assert_eq!(
        Error::from(SysError::IndexOutOfBound),
        Error::SysIndexOutOfBound
    );
    assert_eq!(
        Error::from(SysError::LengthNotEnough(32)),
        Error::SysLengthNotEnough
    );
    assert_eq!(Error::from(SysError::Encoding), Error::SysEncoding);
    assert_eq!(Error::from(SysError::Unknown(255)), Error::SyscallUnknown);
}

#[test]
fn core_errors_map_to_dedicated_contract_errors() {
    assert_eq!(
        Error::from(CoreError::MalformedCobuild),
        Error::MalformedCobuild
    );
    assert_eq!(
        Error::from(CoreError::InvalidOtxLayout),
        Error::InvalidOtxLayout
    );
    assert_eq!(
        Error::from(CoreError::InvalidMessageTarget),
        Error::InvalidMessageTarget
    );
    assert_eq!(
        Error::from(CoreError::DuplicateSighashAll),
        Error::DuplicateSighashAll
    );
    assert_eq!(
        Error::from(CoreError::MissingLockGroupCoverage),
        Error::MissingLockGroupCoverage
    );
    assert_eq!(
        Error::from(CoreError::MissingSealPair),
        Error::MissingSealPair
    );
    assert_eq!(
        Error::from(CoreError::DuplicateSealPair),
        Error::DuplicateSealPair
    );
    assert_eq!(
        Error::from(CoreError::InvalidSealScope),
        Error::InvalidSealScope
    );
    assert_eq!(
        Error::from(CoreError::DuplicateMatchingAction),
        Error::DuplicateMatchingAction
    );
    assert_eq!(
        Error::from(CoreError::InvalidLockGroupWitness),
        Error::InvalidLockGroupWitness
    );
    assert_eq!(
        Error::from(CoreError::InvalidContextInput),
        Error::InvalidContextInput
    );
    assert_eq!(
        Error::from(CoreError::MissingHashInput),
        Error::MissingHashInput
    );
    assert_eq!(
        Error::from(CoreError::HashInputTooLarge),
        Error::HashInputTooLarge
    );
}
