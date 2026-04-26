param(
    [switch]$Fail
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ($Fail) {
    Write-Error "sample benchmark failure"
    exit 1
}

# Replace this sample with your real model invocation.
# The runner exports each CSV field as env var:
#   LLMOS_BENCH_MODEL_NAME, LLMOS_BENCH_CONTEXT_TOKENS, etc.
$model = $env:LLMOS_BENCH_MODEL_NAME
$ctx = [int]$env:LLMOS_BENCH_CONTEXT_TOKENS
$out = [int]$env:LLMOS_BENCH_OUTPUT_TOKENS

$metrics = [ordered]@{
    tokens_per_sec        = [Math]::Round(8.0 + (2048.0 / [Math]::Max($ctx, 1)), 3)
    ttft_ms               = [Math]::Round(120 + ($ctx / 16.0), 2)
    p95_token_latency_ms  = [Math]::Round(35 + ($out / 32.0), 2)
    peak_rss_gb           = [Math]::Round(3.5 + ($ctx / 4096.0), 3)
    swap_used_gb          = 0.0
    psi_mem_some_avg10    = 0.0
    psi_mem_full_avg10    = 0.0
    cpu_avg_pct           = 65
    cpu_peak_pct          = 92
    oom_events            = 0
}

if (-not [string]::IsNullOrWhiteSpace($model)) {
    $metrics["model"] = $model
}

$metrics | ConvertTo-Json -Compress
