use cobuild_otx_lock::error::{Error, ExitCode};

#[test]
fn exit_codes_are_stable() {
    assert_eq!(ExitCode::InvalidArgs as i8, 1);
    assert_eq!(ExitCode::MalformedCobuild as i8, 2);
    assert_eq!(ExitCode::LockSemanticFailure as i8, 3);
    assert_eq!(ExitCode::VerifyFailure as i8, 4);
    assert_eq!(ExitCode::SyscallFailure as i8, 5);
    assert_eq!(ExitCode::InternalFailure as i8, 6);
}

#[test]
fn errors_map_to_stable_exit_codes() {
    assert_eq!(Error::InvalidArgs.exit_code(), ExitCode::InvalidArgs as i8);
    assert_eq!(
        Error::MalformedCobuild.exit_code(),
        ExitCode::MalformedCobuild as i8
    );
    assert_eq!(
        Error::LockSemanticFailure.exit_code(),
        ExitCode::LockSemanticFailure as i8
    );
    assert_eq!(
        Error::VerifyFailure.exit_code(),
        ExitCode::VerifyFailure as i8
    );
    assert_eq!(
        Error::SyscallFailure.exit_code(),
        ExitCode::SyscallFailure as i8
    );
    assert_eq!(
        Error::InternalFailure.exit_code(),
        ExitCode::InternalFailure as i8
    );
}
