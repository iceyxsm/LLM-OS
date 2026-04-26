param(
    [string]$CommandTemplate = $env:LLMOS_REAL_BENCHMARK_COMMAND
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function ConvertTo-HashtableCompat {
    param([object]$Value)

    if ($null -eq $Value) {
        return $null
    }
    if ($Value -is [hashtable]) {
        return $Value
    }

    $map = @{}
    foreach ($prop in $Value.PSObject.Properties) {
        $map[$prop.Name] = $prop.Value
    }
    return $map
}

function Parse-MetricsFromOutput {
    param([string]$OutputText)

    if ([string]::IsNullOrWhiteSpace($OutputText)) {
        return $null
    }

    try {
        $obj = $OutputText | ConvertFrom-Json -ErrorAction Stop
        return ConvertTo-HashtableCompat -Value $obj
    } catch {
    }

    $lines = @($OutputText -split "\r?\n" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    for ($i = $lines.Count - 1; $i -ge 0; $i--) {
        try {
            $obj = $lines[$i] | ConvertFrom-Json -ErrorAction Stop
            return ConvertTo-HashtableCompat -Value $obj
        } catch {
        }
    }

    return $null
}

function Expand-TemplateFromBenchEnv {
    param([string]$Template)

    if ([string]::IsNullOrWhiteSpace($Template)) {
        return ""
    }

    $expanded = $Template

    $fields = @(
        "run_id",
        "date_utc",
        "host_id",
        "ram_class_gb",
        "cpu_model",
        "backend",
        "model_name",
        "model_arch",
        "total_params_b",
        "active_params_b",
        "quantization",
        "context_tokens",
        "output_tokens",
        "concurrency",
        "compression_profile",
        "compression_codec",
        "compression_target_pct",
        "zswap_enabled"
    )

    foreach ($field in $fields) {
        $envKey = "LLMOS_BENCH_{0}" -f $field.ToUpperInvariant()
        $value = [Environment]::GetEnvironmentVariable($envKey)
        if ($null -eq $value) {
            continue
        }
        $expanded = $expanded.Replace(("{" + $field + "}"), [string]$value)
        $expanded = $expanded.Replace(("{" + $envKey + "}"), [string]$value)
    }

    return $expanded
}

function Get-MetricValue {
    param(
        [hashtable]$Metrics,
        [string[]]$Keys,
        [string]$Name
    )

    foreach ($key in $Keys) {
        if ($Metrics.ContainsKey($key) -and -not [string]::IsNullOrWhiteSpace([string]$Metrics[$key])) {
            return $Metrics[$key]
        }
    }
    throw "Missing required metric '$Name'. Checked keys: $($Keys -join ', ')"
}

if ([string]::IsNullOrWhiteSpace($CommandTemplate)) {
    throw "Set -CommandTemplate or env:LLMOS_REAL_BENCHMARK_COMMAND. Example: & 'E:\path\bench.exe' --model {model_name} --ctx {context_tokens} --out {output_tokens}"
}

$commandText = Expand-TemplateFromBenchEnv -Template $CommandTemplate
if ([string]::IsNullOrWhiteSpace($commandText)) {
    throw "Expanded benchmark command is empty."
}

$output = & powershell.exe -NoProfile -Command $commandText 2>&1 | Out-String
$exitCode = if ($null -ne $LASTEXITCODE) { [int]$LASTEXITCODE } else { 0 }
if ($exitCode -ne 0) {
    throw "Benchmark command failed (exit=$exitCode): $($output.Trim())"
}

$parsed = Parse-MetricsFromOutput -OutputText $output
if ($null -eq $parsed) {
    throw "Benchmark command did not output parseable JSON. Output: $($output.Trim())"
}

$normalized = [ordered]@{
    tokens_per_sec       = Get-MetricValue -Metrics $parsed -Keys @("tokens_per_sec", "throughput_tps", "tps") -Name "tokens_per_sec"
    ttft_ms              = Get-MetricValue -Metrics $parsed -Keys @("ttft_ms", "time_to_first_token_ms", "first_token_latency_ms") -Name "ttft_ms"
    p95_token_latency_ms = Get-MetricValue -Metrics $parsed -Keys @("p95_token_latency_ms", "token_latency_p95_ms", "p95_latency_ms") -Name "p95_token_latency_ms"
    peak_rss_gb          = Get-MetricValue -Metrics $parsed -Keys @("peak_rss_gb", "rss_peak_gb") -Name "peak_rss_gb"
    swap_used_gb         = Get-MetricValue -Metrics $parsed -Keys @("swap_used_gb", "swap_gb") -Name "swap_used_gb"
    psi_mem_some_avg10   = Get-MetricValue -Metrics $parsed -Keys @("psi_mem_some_avg10", "psi_some_avg10") -Name "psi_mem_some_avg10"
    psi_mem_full_avg10   = Get-MetricValue -Metrics $parsed -Keys @("psi_mem_full_avg10", "psi_full_avg10") -Name "psi_mem_full_avg10"
    cpu_avg_pct          = Get-MetricValue -Metrics $parsed -Keys @("cpu_avg_pct", "cpu_avg") -Name "cpu_avg_pct"
    cpu_peak_pct         = Get-MetricValue -Metrics $parsed -Keys @("cpu_peak_pct", "cpu_peak") -Name "cpu_peak_pct"
    oom_events           = Get-MetricValue -Metrics $parsed -Keys @("oom_events", "oom_count") -Name "oom_events"
}

$normalized | ConvertTo-Json -Compress
