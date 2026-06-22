#[derive(Default)]
pub struct LocalVerifier;

impl crate::verify::LockVerifier for LocalVerifier {
    fn verify(
        &self,
        auth: &crate::args::AuthContext,
        seal: &[u8],
        signing_message_hash: &[u8; 32],
    ) -> Result<(), crate::error::Error> {
        use k256::ecdsa::{RecoveryId, Signature};

        if seal.len() != 65 {
            return Err(crate::error::Error::VerifyFailure);
        }

        let recovery_id =
            RecoveryId::try_from(seal[64]).map_err(|_| crate::error::Error::VerifyFailure)?;
        let signature =
            Signature::from_slice(&seal[..64]).map_err(|_| crate::error::Error::VerifyFailure)?;
        let public_key =
            super::recover::recover_from_prehash(signing_message_hash, &signature, recovery_id)
                .map_err(|_| crate::error::Error::VerifyFailure)?;
        let encoded = public_key.to_encoded_point(true);
        let pubkey_hash = ckb_hash::blake2b_256(encoded.as_bytes());
        if pubkey_hash[..20] == auth.identity {
            Ok(())
        } else {
            Err(crate::error::Error::VerifyFailure)
        }
    }
}
