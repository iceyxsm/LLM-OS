use crate::parser::parse_benchmark_csv;
use crate::query::{filter_runs, summarize_group, RunFilter};

const SAMPLE_CSV: &str = r#""run_id","date_utc","host_id","ram_class_gb","cpu_model","backend","model_name","model_arch","total_params_b","active_params_b","quantization","context_tokens","output_tokens","concurrency","compression_profile","compression_codec","compression_target_pct","zswap_enabled","tokens_per_sec","ttft_ms","p95_token_latency_ms","peak_rss_gb","swap_used_gb","psi_mem_some_avg10","psi_mem_full_avg10","cpu_avg_pct","cpu_peak_pct","oom_events","run_success","notes"
"R0001","2026-04-26T10:00:00Z","HOST1","8","test-cpu","local","dense-3b","dense","3","3","Q4_K_M","1024","128","1","none","none","0","false","10","184","39","3.75","0","0","0","65","92","0","true","test run"
"R0002","2026-04-26T10:01:00Z","HOST1","8","test-cpu","local","dense-3b","dense","3","3","Q4_K_M","1024","128","1","zram_lz4_25","lz4","25","false","12","170","35","3.5","0","0","0","60","88","0","true","compressed"
"R0003","2026-04-26T10:02:00Z","HOST1","8","test-cpu","local","dense-7b","dense","7","7","Q4_K_M","1024","128","1","none","none","0","false","6","300","55","5.5","0","0","0","70","95","0","true","larger model"
"R0004","2026-04-26T10:03:00Z","HOST1","8","test-cpu","local","dense-3b","dense","3","3","Q4_K_M","4096","256","1","none","none","0","false","8.5","376","43","4.5","0","0","0","65","92","1","false","oom failure"
"#;

#[test]
fn parse_csv_returns_all_records() {
    let records = parse_benchmark_csv(SAMPLE_CSV).unwrap();
    assert_eq!(records.len(), 4);
    assert_eq!(records[0].run_id, "R0001");
    assert_eq!(records[0].model_name, "dense-3b");
    assert_eq!(records[0].tokens_per_sec, 10.0);
    assert!(records[0].run_success);
}

#[test]
fn parse_csv_handles_boolean_fields() {
    let records = parse_benchmark_csv(SAMPLE_CSV).unwrap();
    assert!(!records[0].zswap_enabled);
    assert!(records[0].run_success);
    assert!(!records[3].run_success);
}

#[test]
fn filter_by_model_name() {
    let records = parse_benchmark_csv(SAMPLE_CSV).unwrap();
    let filter = RunFilter {
        model_name: Some("dense-3b".to_string()),
        ..Default::default()
    };
    let filtered = filter_runs(&records, &filter);
    assert_eq!(filtered.len(), 3);
}

#[test]
fn filter_success_only() {
    let records = parse_benchmark_csv(SAMPLE_CSV).unwrap();
    let filter = RunFilter {
        success_only: true,
        ..Default::default()
    };
    let filtered = filter_runs(&records, &filter);
    assert_eq!(filtered.len(), 3);
    assert!(filtered.iter().all(|r| r.run_success));
}

#[test]
fn filter_by_compression_profile() {
    let records = parse_benchmark_csv(SAMPLE_CSV).unwrap();
    let filter = RunFilter {
        compression_profile: Some("none".to_string()),
        success_only: true,
        ..Default::default()
    };
    let filtered = filter_runs(&records, &filter);
    assert_eq!(filtered.len(), 2);
}

#[test]
fn summarize_computes_averages() {
    let records = parse_benchmark_csv(SAMPLE_CSV).unwrap();
    let filter = RunFilter {
        model_name: Some("dense-3b".to_string()),
        success_only: true,
        ..Default::default()
    };
    let filtered = filter_runs(&records, &filter);
    let summary = summarize_group(&filtered);
    assert_eq!(summary.count, 2);
    assert!((summary.avg_tokens_per_sec - 11.0).abs() < 0.01);
    assert_eq!(summary.total_oom_events, 0);
}

#[test]
fn summarize_empty_group() {
    let summary = summarize_group(&[]);
    assert_eq!(summary.count, 0);
    assert_eq!(summary.avg_tokens_per_sec, 0.0);
}

#[test]
fn parse_csv_rejects_missing_header() {
    let result = parse_benchmark_csv("");
    assert!(result.is_err());
}

#[test]
fn parse_csv_rejects_short_rows() {
    let bad = "run_id,date_utc\nR0001,2026-01-01\n";
    let result = parse_benchmark_csv(bad);
    assert!(result.is_err());
}
