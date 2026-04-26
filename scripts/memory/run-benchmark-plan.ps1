param(
    [string]$PlanPath = "scripts/memory/benchmark_plan.csv",
    [string]$BenchmarkCommand = "",
    [string]$BenchmarkScriptPath = "",
    [string]$PreRunCommand = "",
    [string]$PostRunCommand = "",
    [string[]]$RunIds = @(),
    [int]$MaxRuns = 0,
    [switch]$IncludeCompleted,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Expand-Template {
    param(
        [string]$Template,
        [pscustomobject]$Row
    )

    if ([string]::IsNullOrWhiteSpace($Template)) {
        return ""
    }

    $expanded = $Template
    foreach ($prop in $Row.PSObject.Properties) {
        $token = "{0}" -f ("{" + $prop.Name + "}")
        $value = [string]$prop.Value
        $expanded = $expanded.Replace($token, $value)
    }
    return $expanded
}

function Execute-Command {
    param([string]$CommandText)

    if ([string]::IsNullOrWhiteSpace($CommandText)) {
        return [pscustomobject]@{
            ExitCode = 0
            Output   = ""
        }
    }

    $output = & powershell.exe -NoProfile -Command $CommandText 2>&1 | Out-String
    $exitCode = if ($null -ne $LASTEXITCODE) { [int]$LASTEXITCODE } else { 0 }
    return [pscustomobject]@{
        ExitCode = $exitCode
        Output   = $output.Trim()
    }
}

function Parse-MetricsFromOutput {
    param([string]$OutputText)

    if ([string]::IsNullOrWhiteSpace($OutputText)) {
        return $null
    }

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

function Set-EnvFromRow {
    param([pscustomobject]$Row)
    foreach ($prop in $Row.PSObject.Properties) {
        $key = "LLMOS_BENCH_{0}" -f ($prop.Name.ToUpperInvariant())
        Set-Item -Path ("Env:{0}" -f $key) -Value ([string]$prop.Value)
    }
}

function Clear-EnvFromRow {
    param([pscustomobject]$Row)
    foreach ($prop in $Row.PSObject.Properties) {
        $key = "LLMOS_BENCH_{0}" -f ($prop.Name.ToUpperInvariant())
        Remove-Item -Path ("Env:{0}" -f $key) -ErrorAction SilentlyContinue
    }
}

function Update-RowFromMetrics {
    param(
        [pscustomobject]$Row,
        [hashtable]$Metrics
    )

    $metricColumns = @(
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

    foreach ($col in $metricColumns) {
        if ($Metrics.ContainsKey($col)) {
            $Row.$col = [string]$Metrics.$col
        }
    }
}

function Sync-RowBack {
    param(
        [pscustomobject]$Destination,
        [pscustomobject]$Source
    )

    foreach ($prop in $Source.PSObject.Properties) {
        $Destination.($prop.Name) = $Source.($prop.Name)
    }
}

if (-not (Test-Path -LiteralPath $PlanPath)) {
    throw "Plan file not found: $PlanPath"
}

if (-not $DryRun -and [string]::IsNullOrWhiteSpace($BenchmarkCommand) -and [string]::IsNullOrWhiteSpace($BenchmarkScriptPath)) {
    throw "BenchmarkCommand or BenchmarkScriptPath is required unless -DryRun is set."
}

if (-not [string]::IsNullOrWhiteSpace($BenchmarkScriptPath) -and -not (Test-Path -LiteralPath $BenchmarkScriptPath)) {
    throw "BenchmarkScriptPath not found: $BenchmarkScriptPath"
}

$rows = Import-Csv -Path $PlanPath
if ($rows.Count -eq 0) {
    Write-Output "No rows found in $PlanPath"
    exit 0
}

$hostId = $env:COMPUTERNAME
if ([string]::IsNullOrWhiteSpace($hostId)) {
    $hostId = [System.Net.Dns]::GetHostName()
}

$cpuModel = ""
try {
    $cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
    if ($null -ne $cpu) {
        $cpuModel = [string]$cpu.Name
    }
} catch {
    $cpuModel = ""
}

$selectedIndexes = @()
for ($idx = 0; $idx -lt $rows.Count; $idx++) {
    $row = $rows[$idx]
    if ($RunIds.Count -gt 0 -and ($RunIds -notcontains $row.run_id)) {
        continue
    }
    if (-not $IncludeCompleted -and -not [string]::IsNullOrWhiteSpace($row.run_success)) {
        continue
    }
    $selectedIndexes += $idx
}

if ($selectedIndexes.Count -eq 0) {
    Write-Output "No rows selected. Use -IncludeCompleted or -RunIds to adjust selection."
    exit 0
}

$executed = 0
foreach ($rowIndex in $selectedIndexes) {
    if ($MaxRuns -gt 0 -and $executed -ge $MaxRuns) {
        break
    }

    $executed++
    $row = $rows[$rowIndex]
    Write-Output ("[{0}/{1}] run_id={2} model={3} profile={4} ctx={5} out={6}" -f $executed, $selectedIndexes.Count, $row.run_id, $row.model_name, $row.compression_profile, $row.context_tokens, $row.output_tokens)

    $row.date_utc = [DateTime]::UtcNow.ToString("o")
    $row.host_id = $hostId
    if ([string]::IsNullOrWhiteSpace($row.cpu_model)) {
        $row.cpu_model = $cpuModel
    }

    if ($DryRun) {
        $row.run_success = "dry-run"
        if ([string]::IsNullOrWhiteSpace($row.notes)) {
            $row.notes = "dry-run"
        } else {
            $row.notes = "{0};dry-run" -f $row.notes
        }
        Sync-RowBack -Destination $rows[$rowIndex] -Source $row
        $rows | Export-Csv -Path $PlanPath -NoTypeInformation -Encoding utf8
        continue
    }

    Set-EnvFromRow -Row $row
    try {
        $pre = Expand-Template -Template $PreRunCommand -Row $row
        if (-not [string]::IsNullOrWhiteSpace($pre)) {
            $preResult = Execute-Command -CommandText $pre
            if ($preResult.ExitCode -ne 0) {
                $row.run_success = "false"
                $row.notes = "pre-run failed: $($preResult.Output)"
                Sync-RowBack -Destination $rows[$rowIndex] -Source $row
                $rows | Export-Csv -Path $PlanPath -NoTypeInformation -Encoding utf8
                continue
            }
        }

        $benchCmd = ""
        if (-not [string]::IsNullOrWhiteSpace($BenchmarkScriptPath)) {
            $scriptPath = (Resolve-Path -LiteralPath $BenchmarkScriptPath).Path
            $benchCmd = "& '$scriptPath'"
        } else {
            $benchCmd = Expand-Template -Template $BenchmarkCommand -Row $row
        }
        $started = Get-Date
        $result = Execute-Command -CommandText $benchCmd
        $elapsed = (Get-Date) - $started

        if ($result.ExitCode -ne 0) {
            $row.run_success = "false"
            $row.notes = "benchmark failed (exit=$($result.ExitCode)): $($result.Output)"
        } else {
            $metrics = Parse-MetricsFromOutput -OutputText $result.Output
            if ($null -eq $metrics) {
                $row.run_success = "false"
                $row.notes = "benchmark returned no parseable JSON metrics; elapsed_ms=$([int]$elapsed.TotalMilliseconds)"
            } else {
                Update-RowFromMetrics -Row $row -Metrics $metrics
                $row.run_success = "true"
                $row.notes = "elapsed_ms=$([int]$elapsed.TotalMilliseconds)"
            }
        }

        $post = Expand-Template -Template $PostRunCommand -Row $row
        if (-not [string]::IsNullOrWhiteSpace($post)) {
            $null = Execute-Command -CommandText $post
        }
    } finally {
        Clear-EnvFromRow -Row $row
    }

    Sync-RowBack -Destination $rows[$rowIndex] -Source $row
    $rows | Export-Csv -Path $PlanPath -NoTypeInformation -Encoding utf8
}

Write-Output "Completed runs: $executed"
