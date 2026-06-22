use cobuild_otx_lock::args::AuthContext;

#[test]
fn parses_auth_identity() {
    let args = [7u8; 20];
    let auth = AuthContext::try_from(args.as_slice()).unwrap();
    assert_eq!(auth.identity, [7u8; 20]);
}

#[test]
fn rejects_wrong_arg_length() {
    assert!(AuthContext::try_from(&[7u8; 19][..]).is_err());
    assert!(AuthContext::try_from(&[7u8; 21][..]).is_err());
}
