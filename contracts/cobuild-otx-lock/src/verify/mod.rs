pub mod local;
mod recover;

use crate::args::AuthContext;
use crate::error::Error;

pub trait LockVerifier {
    fn verify(
        &self,
        auth: &AuthContext,
        seal: &[u8],
        signing_message_hash: &[u8; 32],
    ) -> Result<(), Error>;
}
