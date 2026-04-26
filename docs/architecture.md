# Architecture

LLM-OS is organized as replaceable modules with strict boundaries.

## Core boundaries
- API contracts: protobuf in `contracts/proto`
- Config contracts: module-local schemas + examples
- Service boundaries: independent binaries communicating over defined interfaces

## Modules

| Module | Status | Implementation |
|---|---|---|
| `core/kernel-profile` | Implemented | `crates/kernel-profile` (llmos-kernel-profile) |
| `core/service-bus` | Implemented | `crates/service-bus` (llmos-service-bus) |
| `core/identity` | Implemented | `crates/identity` (llmos-identity) |
| `core/policy-engine` | Implemented | `services/policy-engine` |
| `core/secrets` | Implemented | `crates/secrets` (llmos-secrets) |
| `runtime/model-runtime` | Planned | -- |
| `runtime/mcp-runtime` | Implemented | `services/mcp-runtime` |
| `runtime/memory-manager` | Planned | -- |
| `security/sandbox` | Implemented | `crates/sandbox` (llmos-sandbox) |
| `security/audit` | Implemented | Audit sinks in `services/llmd` |
| `observability/metrics` | Implemented | Prometheus endpoints in llmd and policy-engine |
| `sdk/plugin-api` | Implemented | Manifest schema + `services/mock-mcp-plugin` |

## Crate library layer

Shared libraries live under `crates/` and are consumed by services and the CLI. This layer provides reusable types and logic that multiple binaries depend on.

- `controlplane-api` -- gRPC/protobuf generated code from `contracts/proto`
- `common-types` -- shared domain types (ActionRequest, AuditEvent, etc.)
- `llmos-secrets` -- secret provider trait with env-based provider and scoped store
- `llmos-identity` -- WorkloadId, IdentityToken, and TokenVerifier
- `llmos-service-bus` -- Transport trait, Envelope, and LocalChannel implementation
- `llmos-kernel-profile` -- MemoryProfile, CgroupDefaults, OomPolicy, and TOML loader
- `llmos-sandbox` -- SeccompProfile, AppArmorProfile, NamespaceConfig, CapabilitySet with presets
- `llmos-benchmark-ingest` -- CSV parser, RunFilter, and RunSummary for benchmark data

## Services

- `services/llmd` -- LLM daemon with policy client, circuit breaker, metrics, audit sinks (JSONL and bus), and secrets integration
- `services/policy-engine` -- policy evaluation with identity-aware subject matching, gRPC interface, and Prometheus metrics
- `services/mcp-runtime` -- MCP plugin lifecycle management with sandbox profile resolution
- `services/mock-mcp-plugin` -- JSON-RPC 2.0 echo server implementing the MCP protocol for testing

## Cross-module integrations

The following integrations connect modules across boundaries:

- **Identity in policy-engine**: The policy engine uses `llmos-identity` for structured WorkloadId subject matching. Subjects are matched by namespace with wildcard support (e.g., `runtime/*` matches all workloads in the runtime namespace).
- **Sandbox in mcp-runtime**: The MCP runtime uses `llmos-sandbox` to resolve seccomp, AppArmor, namespace, and capability profiles per plugin before launching.
- **Kernel profiles in CLI**: The `profile` subcommand uses `llmos-kernel-profile` to load and inspect memory profiles from TOML configuration.
- **Benchmark ingest in CLI**: The `benchmark` subcommand uses `llmos-benchmark-ingest` to parse CSV benchmark data and produce filtered summaries.
- **Service bus in llmd**: The daemon includes a bus audit sink backed by `llmos-service-bus` for publishing audit events to the internal transport.
- **Secrets in llmd**: The daemon integrates `llmos-secrets` for provider token retrieval through the scoped secret store.

## Replaceability rules
- No cross-module database coupling
- Version every external contract
- Capabilities required for plugin and MCP actions
- Default deny for privileged actions
