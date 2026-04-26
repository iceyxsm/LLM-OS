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

## Automated matrix runner
Use the CSV matrix (`scripts/memory/benchmark_plan.csv`) with:

```bash
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/memory/run-benchmark-plan.ps1 \
  -PlanPath scripts/memory/benchmark_plan.csv \
  -BenchmarkScriptPath scripts/memory/sample-benchmark.ps1 \
  -MaxRuns 10
```

Notes:
- The runner exports each row field as env vars prefixed with `LLMOS_BENCH_`.
- Your benchmark script should print one JSON object to stdout with metric keys used in the CSV.
- Optional hooks: `-PreRunCommand` and `-PostRunCommand` support row token expansion, for example `{compression_profile}`.
- `-DryRun` is side-effect free by default; add `-MarkDryRun` if you want to persist `run_success=dry-run` and notes into the CSV.

## Summarize benchmark results
Generate JSON summary statistics from a benchmark plan:

```bash
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/memory/summarize-benchmark-plan.ps1 \
  -PlanPath scripts/memory/benchmark_plan.csv
```

By default, summary metrics are computed from successful rows only (`run_success=true`).
Add `-IncludeFailed` to include all rows in per-profile aggregates.
