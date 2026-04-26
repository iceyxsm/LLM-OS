# LLM-OS Module Registry

This document tracks modules and current ownership.

- `core/kernel-profile`: kernel defaults and memory sysctls
- `core/service-bus`: RPC/event transport abstraction
- `core/identity`: workload identity and auth context
- `core/policy-engine`: central authorization decisions
- `core/secrets`: secret provider abstraction
- `runtime/model-runtime`: provider adapter execution layer
- `runtime/mcp-runtime`: MCP lifecycle and isolation
- `runtime/memory-manager`: memory compression and pressure controls
- `security/sandbox`: seccomp/apparmor templates
- `security/audit`: append-only security events
- `observability/metrics`: metric contract ownership
- `sdk/plugin-api`: plugin manifest and capability model
