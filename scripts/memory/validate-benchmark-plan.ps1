param(
    [string]$PlanPath = "scripts/memory/benchmark_plan.csv"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $PlanPath)) {
    throw "Plan file not found: $PlanPath"
}

$rows = @(Import-Csv -Path $PlanPath)
if ($rows.Count -eq 0) {
    Write-Output ([pscustomobject]@{
            plan_path = $PlanPath
            rows = 0
            status = "ok"
            errors = @()
        } | ConvertTo-Json -Depth 6)
    exit 0
}

$requiredColumns = @(
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
    "zswap_enabled",
    "tokens_per_sec",
    "ttft_ms",
    "p95_token_latency_ms",
    "peak_rss_gb",
    "swap_used_gb",
    "psi_mem_some_avg10",
    "psi_mem_full_avg10",
    "cpu_avg_pct",
    "cpu_peak_pct",
    "oom_events",
    "run_success",
    "notes"
)

$presentColumns = @($rows[0].PSObject.Properties.Name)
$missingColumns = @($requiredColumns | Where-Object { $presentColumns -notcontains $_ })
$errors = New-Object System.Collections.Generic.List[object]

if ($missingColumns.Count -gt 0) {
    $errors.Add([pscustomobject]@{
            type = "missing_columns"
            message = "Missing required columns: $($missingColumns -join ', ')"
        })
}

function Is-Number {
    param([string]$Value)
    if ([string]::IsNullOrWhiteSpace($Value)) {
        return $true
    }
    $parsed = 0.0
    return [double]::TryParse($Value, [ref]$parsed)
}

function Is-BoolString {
    param([string]$Value)
    if ([string]::IsNullOrWhiteSpace($Value)) {
        return $true
    }
    return $Value -eq "true" -or $Value -eq "false"
}

$numericColumns = @(
    "ram_class_gb",
    "total_params_b",
    "active_params_b",
    "context_tokens",
    "output_tokens",
    "concurrency",
    "compression_target_pct",
    "tokens_per_sec",
    "ttft_ms",
    "p95_token_latency_ms",
    "peak_rss_gb",
    "swap_used_gb",
    "psi_mem_some_avg10",
    "psi_mem_full_avg10",
    "cpu_avg_pct",
    "cpu_peak_pct",
    "oom_events"
)

$allowedRunSuccess = @("", "true", "false", "dry-run")
$runIds = New-Object System.Collections.Generic.List[string]

for ($i = 0; $i -lt $rows.Count; $i++) {
    $rowNum = $i + 2
    $row = $rows[$i]

    if ([string]::IsNullOrWhiteSpace($row.run_id)) {
        $errors.Add([pscustomobject]@{
                type = "missing_run_id"
                row = $rowNum
                message = "run_id is required"
            })
    } else {
        $runIds.Add([string]$row.run_id)
    }

    if (-not [string]::IsNullOrWhiteSpace($row.date_utc)) {
        $dt = [datetimeoffset]::MinValue
        if (-not [datetimeoffset]::TryParse([string]$row.date_utc, [ref]$dt)) {
            $errors.Add([pscustomobject]@{
                    type = "invalid_date_utc"
                    row = $rowNum
                    run_id = $row.run_id
                    value = $row.date_utc
                    message = "date_utc is not parseable"
                })
        }
    }

    foreach ($col in $numericColumns) {
        if (-not (Is-Number -Value ([string]$row.$col))) {
            $errors.Add([pscustomobject]@{
                    type = "invalid_numeric_value"
                    row = $rowNum
                    run_id = $row.run_id
                    column = $col
                    value = [string]$row.$col
                    message = "Column '$col' must be numeric when set"
                })
        }
    }

    if (-not (Is-BoolString -Value ([string]$row.zswap_enabled))) {
        $errors.Add([pscustomobject]@{
                type = "invalid_boolean_value"
                row = $rowNum
                run_id = $row.run_id
                column = "zswap_enabled"
                value = [string]$row.zswap_enabled
                message = "zswap_enabled must be 'true' or 'false' when set"
            })
    }

    $runSuccessValue = [string]$row.run_success
    if ($allowedRunSuccess -notcontains $runSuccessValue) {
        $errors.Add([pscustomobject]@{
                type = "invalid_run_success"
                row = $rowNum
                run_id = $row.run_id
                value = $runSuccessValue
                message = "run_success must be one of: '', true, false, dry-run"
            })
    }
}

$duplicateRunIds = @(
    $runIds |
        Group-Object |
        Where-Object { $_.Count -gt 1 } |
        Select-Object -ExpandProperty Name
)
if ($duplicateRunIds.Count -gt 0) {
    $errors.Add([pscustomobject]@{
            type = "duplicate_run_id"
            message = "Duplicate run_id values detected: $($duplicateRunIds -join ', ')"
        })
}

$status = "error"
if ($errors.Count -eq 0) {
    $status = "ok"
}

$report = [pscustomobject]@{
    plan_path = $PlanPath
    rows = $rows.Count
    status = $status
    error_count = $errors.Count
    errors = $errors
}

$report | ConvertTo-Json -Depth 8

if ($errors.Count -gt 0) {
    exit 1
}
