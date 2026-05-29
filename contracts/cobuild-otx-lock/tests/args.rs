use cobuild_otx_lock::args::{parse_auth_args, AUTH_KIND_SECP256K1_BLAKE160};

#[test]
fn parses_auth_kind_and_identity() {
    let mut args = vec![AUTH_KIND_SECP256K1_BLAKE160];
    args.extend_from_slice(&[7u8; 20]);
    let auth = parse_auth_args(&args).unwrap();
    assert_eq!(auth.kind, AUTH_KIND_SECP256K1_BLAKE160);
    assert_eq!(auth.identity, [7u8; 20]);
}

#[test]
fn rejects_wrong_arg_length() {
    assert!(parse_auth_args(&[AUTH_KIND_SECP256K1_BLAKE160]).is_err());
}

#[test]
fn rejects_unsupported_auth_kind() {
    let mut args = vec![AUTH_KIND_SECP256K1_BLAKE160 + 1];
    args.extend_from_slice(&[7u8; 20]);
    assert!(parse_auth_args(&args).is_err());
}
