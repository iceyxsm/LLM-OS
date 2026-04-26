# Kernel Profile Module

Purpose: manage kernel and cgroup defaults for LLM workloads.

## Implementation

The `llmos-kernel-profile` crate in `crates/kernel-profile/` provides:

- `MemoryProfile` for zram/zswap compression settings
- `CgroupDefaults` for memory limits and CPU weight
- `OomPolicy` enum (Kill, Pause, CompressThenKill)
- TOML loader compatible with `config/memory-profiles.toml`
- `resolve_profile` for named profile lookup

## Interfaces
- Inputs: profile name
- Outputs: applied sysctl and cgroup settings
