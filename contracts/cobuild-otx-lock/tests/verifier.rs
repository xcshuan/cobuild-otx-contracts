use cobuild_otx_lock::{
    args::{AuthContext, AUTH_KIND_SECP256K1_BLAKE160},
    verify::{local::LocalVerifier, LockVerifier, VerifyError},
};

struct FailingVerifier;

impl LockVerifier for FailingVerifier {
    fn verify(
        &self,
        _auth: &AuthContext,
        _seal: &[u8],
        _signing_message_hash: &[u8; 32],
    ) -> Result<(), VerifyError> {
        Err(VerifyError::VerificationFailed)
    }
}

#[test]
fn verifier_trait_returns_verify_error() {
    let auth = AuthContext {
        kind: AUTH_KIND_SECP256K1_BLAKE160,
        identity: [0u8; 20],
    };
    assert_eq!(
        FailingVerifier.verify(&auth, &[0u8; 65], &[1u8; 32]),
        Err(VerifyError::VerificationFailed)
    );
}

#[test]
fn local_verifier_rejects_invalid_seal_encoding() {
    let auth = AuthContext {
        kind: AUTH_KIND_SECP256K1_BLAKE160,
        identity: [0u8; 20],
    };
    assert_eq!(
        LocalVerifier.verify(&auth, &[0u8; 64], &[1u8; 32]),
        Err(VerifyError::InvalidSealEncoding)
    );
}

#[test]
fn local_verifier_reports_backend_unavailable_for_valid_seal_shape() {
    let auth = AuthContext {
        kind: AUTH_KIND_SECP256K1_BLAKE160,
        identity: [0u8; 20],
    };
    assert_eq!(
        LocalVerifier.verify(&auth, &[0u8; 65], &[1u8; 32]),
        Err(VerifyError::BackendUnavailable)
    );
}
