# LLM-OS Module Registry

This document tracks modules, their implementation crates, and current status.

## Core modules

### core/kernel-profile
Kernel defaults and memory sysctls.
- **Status**: Implemented
- **Crate**: `crates/kernel-profile` (llmos-kernel-profile)
- **Provides**: MemoryProfile, CgroupDefaults, OomPolicy, TOML loader and resolver

### core/service-bus
RPC/event transport abstraction.
- **Status**: Implemented
- **Crate**: `crates/service-bus` (llmos-service-bus)
- **Provides**: Transport trait, Envelope, LocalChannel

### core/identity
Workload identity and auth context.
- **Status**: Implemented
- **Crate**: `crates/identity` (llmos-identity)
- **Provides**: WorkloadId, IdentityToken, TokenVerifier

### core/policy-engine
Central authorization decisions.
- **Status**: Implemented
- **Service**: `services/policy-engine`
- **Provides**: Policy evaluation with identity-aware subject matching, gRPC interface, Prometheus metrics

### core/secrets
Secret provider abstraction.
- **Status**: Implemented
- **Crate**: `crates/secrets` (llmos-secrets)
- **Provides**: SecretProvider trait, EnvProvider, ScopedStore

## Runtime modules

### runtime/model-runtime
Provider adapter execution layer.
- **Status**: Planned
- **Crate**: --

### runtime/mcp-runtime
MCP lifecycle and isolation.
- **Status**: Implemented
- **Service**: `services/mcp-runtime`
- **Provides**: Plugin lifecycle management, sandbox profile resolution via llmos-sandbox

### runtime/memory-manager
Memory compression and pressure controls.
- **Status**: Planned
- **Crate**: --
- **Notes**: Profile definitions exist in `config/memory-profiles.toml`; scripts in `scripts/memory/`

## Security modules

### security/sandbox
Seccomp/AppArmor templates and namespace configuration.
- **Status**: Implemented
- **Crate**: `crates/sandbox` (llmos-sandbox)
- **Provides**: SeccompProfile, AppArmorProfile, NamespaceConfig, CapabilitySet with presets

### security/audit
Append-only security events.
- **Status**: Implemented
- **Implementation**: Audit sinks in `services/llmd` (JSONL file sink, bus audit sink)

## Observability

### observability/metrics
Metric contract ownership.
- **Status**: Implemented
- **Implementation**: Prometheus endpoints in llmd (`:9090/metrics`) and policy-engine (`:9091/metrics`)
- **Ops**: Docker Compose stack with Prometheus, Grafana, and alert rules under `ops/`

## SDK

### sdk/plugin-api
Plugin manifest and capability model.
- **Status**: Implemented
- **Implementation**: Manifest schema at `sdk/plugin-api/manifest.schema.json`; mock plugin at `services/mock-mcp-plugin`

## Shared crates (no core/ module)

### benchmark-ingest
Benchmark data ingestion and querying.
- **Status**: Implemented
- **Crate**: `crates/benchmark-ingest` (llmos-benchmark-ingest)
- **Provides**: CSV parser, RunFilter, RunSummary
- **Used by**: `llmos-cli` benchmark subcommands

### common-types
Shared domain types used across services and crates.
- **Status**: Implemented
- **Crate**: `crates/common-types`
- **Provides**: ActionRequest, AuditEvent, and other shared types

### controlplane-api
gRPC/protobuf generated code.
- **Status**: Implemented
- **Crate**: `crates/controlplane-api`
- **Provides**: Generated Rust types from `contracts/proto/controlplane/v1/llmos.proto`
