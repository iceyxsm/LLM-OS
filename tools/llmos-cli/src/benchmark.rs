use std::path::PathBuf;

use clap::Args;
use llmos_benchmark_ingest::{filter_runs, parse_benchmark_csv, summarize_group, RunFilter};
use serde_json::json;

use crate::output::{render_json, OutputFormat};

#[derive(Args, Debug)]
pub struct BenchmarkArgs {
    /// Path to the benchmark CSV file.
    #[arg(long, default_value = "scripts/memory/benchmark_plan.csv")]
    pub csv: PathBuf,

    #[command(subcommand)]
    pub command: BenchmarkCommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum BenchmarkCommand {
    /// Summarize benchmark runs with optional filters.
    Summary(SummaryArgs),
    /// List individual benchmark runs.
    List(ListArgs),
}

#[derive(Args, Debug)]
pub struct SummaryArgs {
    /// Filter by model name.
    #[arg(long)]
    pub model: Option<String>,

    /// Filter by compression profile.
    #[arg(long)]
    pub profile: Option<String>,

    /// Filter by context token count.
    #[arg(long)]
    pub context_tokens: Option<u64>,

    /// Only include successful runs.
    #[arg(long, default_value_t = true)]
    pub success_only: bool,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Filter by model name.
    #[arg(long)]
    pub model: Option<String>,

    /// Filter by compression profile.
    #[arg(long)]
    pub profile: Option<String>,

    /// Only include successful runs.
    #[arg(long)]
    pub success_only: bool,

    /// Maximum number of runs to display.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
}

pub fn run_benchmark(args: &BenchmarkArgs) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(&args.csv)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {}", args.csv.display(), e))?;
    let records = parse_benchmark_csv(&content)?;

    match &args.command {
        BenchmarkCommand::Summary(summary_args) => {
            let filter = RunFilter {
                model_name: summary_args.model.clone(),
                compression_profile: summary_args.profile.clone(),
                context_tokens: summary_args.context_tokens,
                success_only: summary_args.success_only,
            };
            let filtered = filter_runs(&records, &filter);
            let summary = summarize_group(&filtered);

            match summary_args.format {
                OutputFormat::Json => {
                    let output = render_json(&json!({
                        "count": summary.count,
                        "avg_tokens_per_sec": summary.avg_tokens_per_sec,
                        "avg_ttft_ms": summary.avg_ttft_ms,
                        "avg_p95_latency_ms": summary.avg_p95_latency_ms,
                        "avg_peak_rss_gb": summary.avg_peak_rss_gb,
                        "total_oom_events": summary.total_oom_events,
                    }))?;
                    println!("{}", output);
                }
                OutputFormat::Text => {
                    println!("Benchmark Summary ({} runs)", summary.count);
                    println!("  avg tokens/sec:     {:.1}", summary.avg_tokens_per_sec);
                    println!("  avg TTFT (ms):      {:.1}", summary.avg_ttft_ms);
                    println!("  avg p95 latency:    {:.1}", summary.avg_p95_latency_ms);
                    println!("  avg peak RSS (GB):  {:.2}", summary.avg_peak_rss_gb);
                    println!("  total OOM events:   {}", summary.total_oom_events);
                }
            }
        }
        BenchmarkCommand::List(list_args) => {
            let filter = RunFilter {
                model_name: list_args.model.clone(),
                compression_profile: list_args.profile.clone(),
                context_tokens: None,
                success_only: list_args.success_only,
            };
            let filtered = filter_runs(&records, &filter);
            let display: Vec<_> = filtered.iter().take(list_args.limit).collect();

            match list_args.format {
                OutputFormat::Json => {
                    let entries: Vec<serde_json::Value> = display
                        .iter()
                        .map(|r| {
                            json!({
                                "run_id": r.run_id,
                                "model_name": r.model_name,
                                "compression_profile": r.compression_profile,
                                "context_tokens": r.context_tokens,
                                "tokens_per_sec": r.tokens_per_sec,
                                "peak_rss_gb": r.peak_rss_gb,
                                "run_success": r.run_success,
                            })
                        })
                        .collect();
                    let output = render_json(&json!({ "runs": entries }))?;
                    println!("{}", output);
                }
                OutputFormat::Text => {
                    println!(
                        "{:<8} {:<20} {:<18} {:>6} {:>8} {:>8} {:>7}",
                        "RUN_ID", "MODEL", "PROFILE", "CTX", "TPS", "RSS_GB", "OK"
                    );
                    for r in &display {
                        println!(
                            "{:<8} {:<20} {:<18} {:>6} {:>8.1} {:>8.2} {:>7}",
                            r.run_id,
                            r.model_name,
                            r.compression_profile,
                            r.context_tokens,
                            r.tokens_per_sec,
                            r.peak_rss_gb,
                            r.run_success,
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn summary_json_includes_expected_keys() {
        let output = json!({
            "count": 5,
            "avg_tokens_per_sec": 10.0,
            "avg_ttft_ms": 200.0,
            "avg_p95_latency_ms": 40.0,
            "avg_peak_rss_gb": 3.5,
            "total_oom_events": 0,
        });
        assert!(output.get("count").is_some());
        assert!(output.get("avg_tokens_per_sec").is_some());
        assert!(output.get("total_oom_events").is_some());
    }

    #[test]
    fn list_json_includes_expected_keys() {
        let entry = json!({
            "run_id": "R0001",
            "model_name": "dense-3b",
            "compression_profile": "none",
            "context_tokens": 1024,
            "tokens_per_sec": 10.0,
            "peak_rss_gb": 3.75,
            "run_success": true,
        });
        assert!(entry.get("run_id").is_some());
        assert!(entry.get("tokens_per_sec").is_some());
    }
}
