use std::time::{SystemTime, UNIX_EPOCH};

use crate::token::{IdentityToken, TokenClaims};
use crate::verifier::{TokenVerifier, VerificationError};
use crate::workload::WorkloadId;

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn valid_claims() -> TokenClaims {
    let now = now_ms();
    TokenClaims {
        subject: WorkloadId::parse("runtime/model-runtime").unwrap(),
        capabilities: vec!["network:egress".to_string()],
        issued_at_unix_ms: now - 1_000,
        expires_at_unix_ms: now + 60_000,
    }
}

#[test]
fn workload_id_parse_valid() {
    let id = WorkloadId::parse("runtime/model-runtime").unwrap();
    assert_eq!(id.namespace(), "runtime");
    assert_eq!(id.name(), "model-runtime");
    assert_eq!(id.to_string(), "runtime/model-runtime");
}

#[test]
fn workload_id_parse_rejects_missing_slash() {
    assert!(WorkloadId::parse("no-slash").is_err());
}

#[test]
fn workload_id_parse_rejects_empty_segments() {
    assert!(WorkloadId::parse("/name").is_err());
    assert!(WorkloadId::parse("namespace/").is_err());
}

#[test]
fn workload_id_parse_rejects_uppercase() {
    assert!(WorkloadId::parse("Runtime/Model").is_err());
}

#[test]
fn token_roundtrip() {
    let claims = valid_claims();
    let token = IdentityToken::issue(claims.clone()).unwrap();
    let decoded = IdentityToken::decode(token.raw()).unwrap();
    assert_eq!(decoded.claims().subject, claims.subject);
    assert_eq!(decoded.claims().capabilities, claims.capabilities);
}

#[test]
fn verify_valid_token() {
    let token = IdentityToken::issue(valid_claims()).unwrap();
    let verified = TokenVerifier::verify(token.raw()).unwrap();
    assert_eq!(
        verified.claims().subject,
        WorkloadId::parse("runtime/model-runtime").unwrap()
    );
}

#[test]
fn verify_expired_token() {
    let now = now_ms();
    let claims = TokenClaims {
        subject: WorkloadId::parse("runtime/model-runtime").unwrap(),
        capabilities: vec![],
        issued_at_unix_ms: now - 120_000,
        expires_at_unix_ms: now - 60_000,
    };
    let token = IdentityToken::issue(claims).unwrap();
    let err = TokenVerifier::verify(token.raw()).unwrap_err();
    assert!(matches!(err, VerificationError::Expired));
}

#[test]
fn verify_for_wrong_subject() {
    let token = IdentityToken::issue(valid_claims()).unwrap();
    let wrong = WorkloadId::parse("runtime/mcp-runtime").unwrap();
    let err = TokenVerifier::verify_for(token.raw(), &wrong).unwrap_err();
    assert!(matches!(err, VerificationError::SubjectMismatch { .. }));
}

#[test]
fn verify_for_correct_subject() {
    let token = IdentityToken::issue(valid_claims()).unwrap();
    let expected = WorkloadId::parse("runtime/model-runtime").unwrap();
    TokenVerifier::verify_for(token.raw(), &expected).unwrap();
}
