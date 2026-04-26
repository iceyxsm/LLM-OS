# Identity Module

Purpose: authenticate workloads and issue scoped identities.

## Implementation

The `llmos-identity` crate in `crates/identity/` provides:

- `WorkloadId` structured identifier following the `namespace/name` convention
- `IdentityToken` and `TokenClaims` for issuing and decoding workload tokens
- `TokenVerifier` for validating token expiry and subject assertions

## Responsibilities
- Workload identity issuance
- Service-to-service auth context
- Key and token verification hooks
