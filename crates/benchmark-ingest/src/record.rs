use serde::{Deserialize, Serialize};

/// A single benchmark run record matching the CSV schema in
/// `scripts/memory/benchmark_plan.csv`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRecord {
    pub run_id: String,
    pub date_utc: String,
    pub host_id: String,
    pub ram_class_gb: f64,
    pub cpu_model: String,
    pub backend: String,
    pub model_name: String,
    pub model_arch: String,
    pub total_params_b: f64,
    pub active_params_b: f64,
    pub quantization: String,
    pub context_tokens: u64,
    pub output_tokens: u64,
    pub concurrency: u32,
    pub compression_profile: String,
    pub compression_codec: String,
    pub compression_target_pct: u32,
    pub zswap_enabled: bool,
    pub tokens_per_sec: f64,
    pub ttft_ms: f64,
    pub p95_token_latency_ms: f64,
    pub peak_rss_gb: f64,
    pub swap_used_gb: f64,
    pub psi_mem_some_avg10: f64,
    pub psi_mem_full_avg10: f64,
    pub cpu_avg_pct: f64,
    pub cpu_peak_pct: f64,
    pub oom_events: u32,
    pub run_success: bool,
    pub notes: String,
}
