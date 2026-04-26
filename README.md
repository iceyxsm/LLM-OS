# LLM-OS

A modular operating environment focused on running LLM workloads with strong isolation, policy controls, MCP integration, and memory-compression experiments.

## Design goals
- Modular services with versioned contracts
- Security-first execution (sandbox + policy + audit)
- Provider-agnostic LLM runtime (API and local backends)
- Built-in MCP lifecycle management
- Memory-efficiency experiments (`zram`/`zswap`)

## Repository layout
- `crates/`: shared Rust libraries consumed by services and tools
- `core/`: foundational platform module specifications
- `runtime/`: model, MCP, and memory runtime modules
- `security/`: sandboxing and audit components
- `observability/`: metrics and telemetry definitions
- `services/`: runnable daemon implementations
- `tools/`: operator CLI
- `contracts/`: protobuf and API contracts
- `sdk/`: plugin API and schemas
- `config/`: daemon, profile, and policy configuration files
- `scripts/`: operational and experiment scripts
- `ops/`: Docker Compose, Prometheus, and Grafana stack

## Crate library

Shared libraries live under `crates/`. Each crate is an independent Cargo package.

| Crate | Package | Purpose |
|---|---|---|
| `crates/controlplane-api` | `controlplane-api` | gRPC/protobuf contract definitions |
| `crates/common-types` | `common-types` | Shared types (ActionRequest, AuditEvent, etc.) |
| `crates/secrets` | `llmos-secrets` | Secret provider trait, env provider, scoped store |
| `crates/identity` | `llmos-identity` | WorkloadId, IdentityToken, TokenVerifier |
| `crates/service-bus` | `llmos-service-bus` | Transport trait, Envelope, LocalChannel |
| `crates/kernel-profile` | `llmos-kernel-profile` | MemoryProfile, CgroupDefaults, OomPolicy, TOML loader |
| `crates/sandbox` | `llmos-sandbox` | SeccompProfile, AppArmorProfile, NamespaceConfig, CapabilitySet |
| `crates/benchmark-ingest` | `llmos-benchmark-ingest` | CSV parser, RunFilter, RunSummary for benchmark data |

## Quick start
1. Install Rust stable (`rustup`)
2. Build all modules:
   ```bash
   cargo build --workspace
   ```
3. Run starter services:
   ```bash
   cargo run -p llmd
   cargo run -p mcp-runtime
   cargo run -p policy-engine
   ```
4. Use the operator CLI:
   ```bash
   cargo run -p llmos-cli -- modules
   cargo run -p llmos-cli -- policy health
   cargo run -p llmos-cli -- profile list
   cargo run -p llmos-cli -- benchmark summary
   ```

## Operator commands

### Module status
```bash
cargo run -p llmos-cli -- modules
```

### Policy
- Policy decision check:
  ```bash
  cargo run -p llmos-cli -- policy check --subject runtime/model-runtime --action network:connect --resource api.openai.com
  ```
- Policy health check:
  ```bash
  cargo run -p llmos-cli -- policy health
  ```

### Kernel profiles
- List available memory profiles:
  ```bash
  cargo run -p llmos-cli -- profile list
  ```
- Show details for a specific profile:
  ```bash
  cargo run -p llmos-cli -- profile show balanced
  ```

### Benchmark data
- Summarize benchmark runs (with optional filters):
  ```bash
  cargo run -p llmos-cli -- benchmark summary
  ```
- List individual benchmark runs:
  ```bash
  cargo run -p llmos-cli -- benchmark list
  ```

## Audit log rotation
When `LLMOS_AUDIT_JSONL_PATH` is set, `llmd` writes JSONL audit events and rotates by size.
- `LLMOS_AUDIT_ROTATE_MAX_BYTES` default: `10485760` (10 MB)
- `LLMOS_AUDIT_ROTATE_MAX_FILES` default: `5`

## Metrics endpoint
`llmd` exposes Prometheus-compatible metrics at `GET /metrics`.
- `LLMOS_METRICS_LISTEN` default: `127.0.0.1:9090`
- Example:
  ```bash
  curl http://127.0.0.1:9090/metrics
  ```
`policy-engine` also exposes Prometheus-compatible metrics at `GET /metrics`.
- `LLMOS_POLICY_METRICS_LISTEN` default: `127.0.0.1:9091`
- Example:
  ```bash
  curl http://127.0.0.1:9091/metrics
  ```

## Ops package
Monitoring stack files live under [`ops/`](ops/README.md):
- Prometheus scrape config and alert rules
- Grafana dashboard JSON
- Docker Compose for local observability stack
- Alert rules for policy breaker, unavailability, and deny spikes

## Memory compression experiments
See `runtime/memory-manager/README.md` and `scripts/memory/` for zram profile application and benchmark capture.

## Configuration
- `config/llmos.toml` -- daemon configuration
- `config/memory-profiles.toml` -- zram/zswap profiles (balanced, aggressive, low_latency)
- `config/policy.example.yaml` -- example policy rules

## Notes
This workspace contains 13 crates: 8 shared libraries, 3 services, 1 mock plugin, and 1 CLI tool. Core modules under `core/` have corresponding implementations in `crates/` and `services/`. The architecture is designed for independent replacement of any module without cross-module coupling.
