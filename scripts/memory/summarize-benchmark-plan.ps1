param(
    [string]$PlanPath = "scripts/memory/benchmark_plan.csv",
    [switch]$IncludeFailed
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Try-ParseDouble {
    param([string]$Value)
    $parsed = 0.0
    if ([double]::TryParse($Value, [ref]$parsed)) {
        return $parsed
    }
    return $null
}

function Get-Median {
    param([double[]]$Values)
    if ($null -eq $Values -or $Values.Count -eq 0) {
        return $null
    }
    $sorted = @($Values | Sort-Object)
    $count = $sorted.Count
    if ($count % 2 -eq 1) {
        return $sorted[[int](($count - 1) / 2)]
    }
    $mid = [int]($count / 2)
    return ($sorted[$mid - 1] + $sorted[$mid]) / 2.0
}

if (-not (Test-Path -LiteralPath $PlanPath)) {
    throw "Plan file not found: $PlanPath"
}

$rows = @(Import-Csv -Path $PlanPath)
if ($rows.Count -eq 0) {
    Write-Output "{}"
    exit 0
}

$total = $rows.Count
$successfulRows = @($rows | Where-Object { $_.run_success -eq "true" })
$failedRows = @($rows | Where-Object {
    -not [string]::IsNullOrWhiteSpace($_.run_success) -and $_.run_success -ne "true"
})
$pendingRows = @($rows | Where-Object { [string]::IsNullOrWhiteSpace($_.run_success) })

$analyzableRows = if ($IncludeFailed) { $rows } else { $successfulRows }

$profiles = @(
    $analyzableRows |
        Group-Object compression_profile |
        Sort-Object Name |
        ForEach-Object {
            $groupRows = $_.Group
            $tps = @($groupRows | ForEach-Object { Try-ParseDouble -Value $_.tokens_per_sec } | Where-Object { $null -ne $_ })
            $ttft = @($groupRows | ForEach-Object { Try-ParseDouble -Value $_.ttft_ms } | Where-Object { $null -ne $_ })
            $peakRss = @($groupRows | ForEach-Object { Try-ParseDouble -Value $_.peak_rss_gb } | Where-Object { $null -ne $_ })
            $swap = @($groupRows | ForEach-Object { Try-ParseDouble -Value $_.swap_used_gb } | Where-Object { $null -ne $_ })
            $oomSum = @($groupRows | ForEach-Object { Try-ParseDouble -Value $_.oom_events } | Where-Object { $null -ne $_ } | Measure-Object -Sum).Sum

            [pscustomobject]@{
                compression_profile = $_.Name
                runs = $groupRows.Count
                avg_tokens_per_sec = if ($tps.Count -gt 0) { [Math]::Round((($tps | Measure-Object -Average).Average), 3) } else { $null }
                median_ttft_ms = if ($ttft.Count -gt 0) { [Math]::Round((Get-Median -Values $ttft), 3) } else { $null }
                median_peak_rss_gb = if ($peakRss.Count -gt 0) { [Math]::Round((Get-Median -Values $peakRss), 3) } else { $null }
                median_swap_used_gb = if ($swap.Count -gt 0) { [Math]::Round((Get-Median -Values $swap), 3) } else { $null }
                oom_events_sum = if ($null -ne $oomSum) { [int]$oomSum } else { 0 }
            }
        }
)

$summary = [ordered]@{
    plan_path = $PlanPath
    include_failed = [bool]$IncludeFailed
    total_rows = $total
    successful_rows = $successfulRows.Count
    failed_rows = $failedRows.Count
    pending_rows = $pendingRows.Count
    success_rate_pct = if (($successfulRows.Count + $failedRows.Count) -gt 0) {
        [Math]::Round((100.0 * $successfulRows.Count / ($successfulRows.Count + $failedRows.Count)), 2)
    } else {
        $null
    }
    profile_summary = $profiles
}

$summary | ConvertTo-Json -Depth 6
