# Architecture

LLM-OS is organized as replaceable modules with strict boundaries.

## Core boundaries
- API contracts: protobuf in `contracts/proto`
- Config contracts: module-local schemas + examples
- Service boundaries: independent binaries communicating over defined interfaces

## Modules
- `core/kernel-profile`: kernel and cgroup profile definitions
- `core/service-bus`: transport abstraction (gRPC/NATS swap-ready)
- `core/identity`: workload identity and auth primitives
- `core/policy-engine`: policy decision point for tool/file/network access
- `core/secrets`: secure secret retrieval and provider token management
- `runtime/model-runtime`: provider adapters and inference orchestration
- `runtime/mcp-runtime`: MCP server lifecycle and sandbox hooks
- `runtime/memory-manager`: zram/zswap profiles and pressure management
- `security/sandbox`: seccomp/apparmor profile templates
- `security/audit`: append-only audit event definitions
- `observability/metrics`: metric names and dashboards
- `sdk/plugin-api`: plugin manifest schema and capability model

## Replaceability rules
- No cross-module database coupling
- Version every external contract
- Capabilities required for plugin and MCP actions
- Default deny for privileged actions
