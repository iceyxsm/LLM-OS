# LLM-OS

A modular operating environment focused on running LLM workloads with strong isolation, policy controls, MCP integration, and memory-compression experiments.

## Design goals
- Modular services with versioned contracts
- Security-first execution (sandbox + policy + audit)
- Provider-agnostic LLM runtime (API and local backends)
- Built-in MCP lifecycle management
- Memory-efficiency experiments (`zram`/`zswap`)

## Repository layout
- `core/`: foundational platform modules
- `runtime/`: model, MCP, and memory runtime modules
- `security/`: sandboxing and audit components
- `observability/`: metrics and telemetry definitions
- `services/`: runnable daemon implementations
- `tools/`: operator CLI
- `contracts/`: protobuf and API contracts
- `sdk/`: plugin API and schemas
- `scripts/`: operational and experiment scripts

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
   cargo run -p llmos-cli -- modules
   ```

## Operator commands
- Policy decision check:
  ```bash
  cargo run -p llmos-cli -- policy check --subject runtime/model-runtime --action network:connect --resource api.openai.com
  ```
- Policy health check:
  ```bash
  cargo run -p llmos-cli -- policy health
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

## Memory compression experiments
See `runtime/memory-manager/README.md` and `scripts/memory/` for zram profile application and benchmark capture.

## Notes
This is a scaffold for rapid experimentation. Contracts and modules are intentionally small, versioned, and replaceable.
