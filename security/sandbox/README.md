# Sandbox Module

Purpose: enforce process isolation boundaries.

## Implementation

The `llmos-sandbox` crate in `crates/sandbox/` provides:

- `SeccompProfile` and `SeccompTemplate` with pre-built profiles for MCP plugins and model runtime
- `AppArmorProfile` and `AppArmorTemplate` with path-based read/write/network rules
- `NamespaceConfig` and `NamespacePreset` for Linux namespace isolation (full, network-only, none)
- `CapabilitySet` and `CapabilityPreset` for Linux capability management

## Responsibilities
- seccomp templates
- AppArmor profile templates
- Namespace and capability presets
