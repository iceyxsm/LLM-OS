use std::time::{SystemTime, UNIX_EPOCH};

use crate::token::IdentityToken;
use crate::workload::WorkloadId;

/// Errors returned when token verification fails.
#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("token has expired")]
    Expired,
    #[error("token is not yet valid")]
    NotYetValid,
    #[error("token decode failed: {0}")]
    DecodeFailed(String),
    #[error("subject mismatch: expected {expected}, got {actual}")]
    SubjectMismatch { expected: String, actual: String },
}

/// Verifies identity tokens.
///
/// In a production system this would validate cryptographic signatures.
/// The current implementation checks expiry, validity window, and
/// optionally asserts the subject matches an expected workload id.
pub struct TokenVerifier;

impl TokenVerifier {
    /// Verify a raw token string, returning the decoded token on success.
    pub fn verify(raw: &str) -> Result<IdentityToken, VerificationError> {
        let token = IdentityToken::decode(raw)
            .map_err(|e| VerificationError::DecodeFailed(e.to_string()))?;

        let now = now_unix_millis();
        let claims = token.claims();

        if now < claims.issued_at_unix_ms {
            return Err(VerificationError::NotYetValid);
        }

        if now >= claims.expires_at_unix_ms {
            return Err(VerificationError::Expired);
        }

        Ok(token)
    }

    /// Verify a token and assert that its subject matches the expected workload.
    pub fn verify_for(
        raw: &str,
        expected: &WorkloadId,
    ) -> Result<IdentityToken, VerificationError> {
        let token = Self::verify(raw)?;
        let actual = &token.claims().subject;

        if actual != expected {
            return Err(VerificationError::SubjectMismatch {
                expected: expected.to_string(),
                actual: actual.to_string(),
            });
        }

        Ok(token)
    }

    /// Verify a signed token string, checking both HMAC signature and expiry.
    pub fn verify_signed(raw: &str, key: &[u8]) -> Result<IdentityToken, VerificationError> {
        let token = IdentityToken::decode_signed(raw, key)
            .map_err(|e| VerificationError::DecodeFailed(e.to_string()))?;

        let now = now_unix_millis();
        let claims = token.claims();

        if now < claims.issued_at_unix_ms {
            return Err(VerificationError::NotYetValid);
        }

        if now >= claims.expires_at_unix_ms {
            return Err(VerificationError::Expired);
        }

        Ok(token)
    }
}

fn now_unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}
