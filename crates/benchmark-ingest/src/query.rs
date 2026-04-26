use crate::record::BenchmarkRecord;

/// Filter criteria for selecting benchmark runs.
#[derive(Debug, Default)]
pub struct RunFilter {
    pub model_name: Option<String>,
    pub compression_profile: Option<String>,
    pub context_tokens: Option<u64>,
    pub success_only: bool,
}

/// Summary statistics for a group of benchmark runs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RunSummary {
    pub count: usize,
    pub avg_tokens_per_sec: f64,
    pub avg_ttft_ms: f64,
    pub avg_p95_latency_ms: f64,
    pub avg_peak_rss_gb: f64,
    pub total_oom_events: u32,
}

/// Filter benchmark records by the given criteria.
pub fn filter_runs<'a>(
    records: &'a [BenchmarkRecord],
    filter: &RunFilter,
) -> Vec<&'a BenchmarkRecord> {
    records
        .iter()
        .filter(|r| {
            if filter.success_only && !r.run_success {
                return false;
            }
            if let Some(ref name) = filter.model_name {
                if r.model_name != *name {
                    return false;
                }
            }
            if let Some(ref profile) = filter.compression_profile {
                if r.compression_profile != *profile {
                    return false;
                }
            }
            if let Some(ctx) = filter.context_tokens {
                if r.context_tokens != ctx {
                    return false;
                }
            }
            true
        })
        .collect()
}

/// Compute summary statistics for a slice of benchmark records.
pub fn summarize_group(records: &[&BenchmarkRecord]) -> RunSummary {
    if records.is_empty() {
        return RunSummary {
            count: 0,
            avg_tokens_per_sec: 0.0,
            avg_ttft_ms: 0.0,
            avg_p95_latency_ms: 0.0,
            avg_peak_rss_gb: 0.0,
            total_oom_events: 0,
        };
    }

    let count = records.len();
    let sum_tps: f64 = records.iter().map(|r| r.tokens_per_sec).sum();
    let sum_ttft: f64 = records.iter().map(|r| r.ttft_ms).sum();
    let sum_p95: f64 = records.iter().map(|r| r.p95_token_latency_ms).sum();
    let sum_rss: f64 = records.iter().map(|r| r.peak_rss_gb).sum();
    let total_oom: u32 = records.iter().map(|r| r.oom_events).sum();

    RunSummary {
        count,
        avg_tokens_per_sec: sum_tps / count as f64,
        avg_ttft_ms: sum_ttft / count as f64,
        avg_p95_latency_ms: sum_p95 / count as f64,
        avg_peak_rss_gb: sum_rss / count as f64,
        total_oom_events: total_oom,
    }
}
