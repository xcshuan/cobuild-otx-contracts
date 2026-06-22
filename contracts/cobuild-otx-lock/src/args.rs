pub const AUTH_ARGS_LEN: usize = 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthContext {
    pub identity: [u8; 20],
}

impl TryFrom<&[u8]> for AuthContext {
    type Error = crate::error::Error;

    fn try_from(args: &[u8]) -> Result<Self, Self::Error> {
        if args.len() != AUTH_ARGS_LEN {
            return Err(crate::error::Error::InvalidArgs);
        }

        let mut identity = [0u8; 20];
        identity.copy_from_slice(args);

        Ok(Self { identity })
    }
}
