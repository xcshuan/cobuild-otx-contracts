#[derive(Default)]
pub struct LocalVerifier;

impl crate::verify::LockVerifier for LocalVerifier {
    fn verify(
        &self,
        _auth: &crate::args::AuthContext,
        seal: &[u8],
        _signing_message_hash: &[u8; 32],
    ) -> Result<(), crate::verify::VerifyError> {
        if seal.len() != 65 {
            return Err(crate::verify::VerifyError::InvalidSealEncoding);
        }

        Err(crate::verify::VerifyError::BackendUnavailable)
    }
}
