#[derive(Default)]
pub struct LocalVerifier;

impl crate::verify::LockVerifier for LocalVerifier {
    fn verify(
        &self,
        auth: &crate::args::AuthContext,
        seal: &[u8],
        signing_message_hash: &[u8; 32],
    ) -> Result<(), crate::error::Error> {
        use secp256k1::{Message, Secp256k1, ecdsa};

        if seal.len() != 65 {
            return Err(crate::error::Error::VerifyFailure);
        }

        if auth.kind != crate::args::AUTH_KIND_SECP256K1_BLAKE160 {
            return Err(crate::error::Error::VerifyFailure);
        }

        let recovery_id = ecdsa::RecoveryId::try_from(i32::from(seal[64]))
            .map_err(|_| crate::error::Error::VerifyFailure)?;
        let signature = ecdsa::RecoverableSignature::from_compact(&seal[..64], recovery_id)
            .map_err(|_| crate::error::Error::VerifyFailure)?;
        let message = Message::from_digest(*signing_message_hash);
        let secp = Secp256k1::verification_only();
        let public_key = secp
            .recover_ecdsa(&message, &signature)
            .map_err(|_| crate::error::Error::VerifyFailure)?;
        let pubkey_hash = ckb_hash::blake2b_256(public_key.serialize());
        if pubkey_hash[..20] == auth.identity {
            Ok(())
        } else {
            Err(crate::error::Error::VerifyFailure)
        }
    }
}
