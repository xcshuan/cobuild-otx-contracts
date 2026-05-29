pub mod local;

use crate::args::AuthContext;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifyError {
    InvalidSealEncoding,
    VerificationFailed,
    BackendUnavailable,
}

pub trait LockVerifier {
    fn verify(
        &self,
        auth: &AuthContext,
        seal: &[u8],
        signing_message_hash: &[u8; 32],
    ) -> Result<(), VerifyError>;
}
