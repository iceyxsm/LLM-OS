# Memory Manager Module

Purpose: optimize memory usage with compression-aware policies.

## Responsibilities
- Apply zram/zswap profiles
- Expose pressure feedback loops
- Track compression and swap metrics

## Profiles
- `balanced`: moderate compression and latency tradeoff
- `aggressive`: high compression pressure
- `low-latency`: reduced compression overhead

## Runbook
1. Apply profile via `scripts/memory/apply-zram-profile.sh <profile>`
2. Capture metrics via `scripts/memory/benchmark.sh`
3. Compare against no-zram baseline
