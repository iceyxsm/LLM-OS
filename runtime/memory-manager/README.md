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

## Real benchmark script
Use `scripts/memory/real-benchmark.ps1` to wrap your actual inference command.
Set `LLMOS_REAL_BENCHMARK_COMMAND` once, then run the matrix with `-BenchmarkScriptPath`.

Example:

```powershell
$env:LLMOS_REAL_BENCHMARK_COMMAND = "& 'E:\path\to\benchmark.exe' --model {model_name} --ctx {context_tokens} --out {output_tokens}"
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/memory/run-benchmark-plan.ps1 `
  -PlanPath scripts/memory/benchmark_plan.csv `
  -BenchmarkScriptPath scripts/memory/real-benchmark.ps1 `
  -IncludeCompleted
```

Template placeholders accept either CSV field names (`{model_name}`, `{context_tokens}`, ...)
or full env keys (`{LLMOS_BENCH_MODEL_NAME}`, `{LLMOS_BENCH_CONTEXT_TOKENS}`, ...).

## Summarize benchmark results
Generate JSON summary statistics from a benchmark plan:

```bash
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/memory/summarize-benchmark-plan.ps1 \
  -PlanPath scripts/memory/benchmark_plan.csv
```

By default, summary metrics are computed from successful rows only (`run_success=true`).
Add `-IncludeFailed` to include all rows in per-profile aggregates.

## Validate benchmark plan
Validate CSV shape and data types before or after runs:

```bash
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/memory/validate-benchmark-plan.ps1 \
  -PlanPath scripts/memory/benchmark_plan.csv
```

The validator checks required columns, duplicate `run_id`, numeric field formats,
boolean fields, and parseable `date_utc` values.

## Default profile policy
Current tier defaults are codified at:

- `runtime/memory-manager/profiles/default-compression-policy.json`

This mapping was derived from:

1. Full matrix execution with `scripts/memory/run-benchmark-plan.ps1`
2. Aggregate scoring on `median_ttft_ms`, `avg_tokens_per_sec`, and `median_peak_rss_gb`

Recompute after collecting real benchmark data (instead of `sample-benchmark.ps1`) before using these defaults in production.
