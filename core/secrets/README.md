# Secrets Module

Purpose: secure retrieval and rotation of provider credentials.

## Implementation

The `llmos-secrets` crate in `crates/secrets/` provides:

- `SecretProvider` trait for pluggable backends (env vars, vaults, files)
- `EnvSecretProvider` for environment-variable-based secrets with configurable prefix
- `SecretStore` with per-module access control scoping
- `ScopedSecretStore` convenience wrapper for binding a store to a module

## Responsibilities
- Encrypted storage integration
- Token lease support
- Per-module secret scoping
