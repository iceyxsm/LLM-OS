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

## Memory compression experiments
See `runtime/memory-manager/README.md` and `scripts/memory/` for zram profile application and benchmark capture.

## Notes
This is a scaffold for rapid experimentation. Contracts and modules are intentionally small, versioned, and replaceable.
