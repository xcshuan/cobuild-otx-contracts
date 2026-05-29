pub const AUTH_KIND_SECP256K1_BLAKE160: u8 = 0;
pub const AUTH_ARGS_LEN: usize = 21;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthContext {
    pub kind: u8,
    pub identity: [u8; 20],
}

pub fn parse_auth_args(args: &[u8]) -> Result<AuthContext, crate::error::Error> {
    if args.len() != AUTH_ARGS_LEN {
        return Err(crate::error::Error::InvalidArgs);
    }
    if args[0] != AUTH_KIND_SECP256K1_BLAKE160 {
        return Err(crate::error::Error::InvalidArgs);
    }

    let mut identity = [0u8; 20];
    identity.copy_from_slice(&args[1..]);

    Ok(AuthContext {
        kind: args[0],
        identity,
    })
}
