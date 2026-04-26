use anyhow::{bail, Context, Result};

use crate::record::BenchmarkRecord;

/// Parse benchmark records from CSV content.
///
/// Expects a header row followed by data rows. Fields are comma-separated
/// and optionally quoted. The field order must match the benchmark_plan.csv
/// schema.
pub fn parse_benchmark_csv(content: &str) -> Result<Vec<BenchmarkRecord>> {
    let mut lines = content.lines();

    let header = lines
        .next()
        .context("CSV is empty; expected a header row")?;
    validate_header(header)?;

    let mut records = Vec::new();
    for (line_num, line) in lines.enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record = parse_row(trimmed)
            .with_context(|| format!("failed to parse CSV row {}", line_num + 2))?;
        records.push(record);
    }

    Ok(records)
}

fn validate_header(header: &str) -> Result<()> {
    let fields = split_csv_row(header);
    if fields.len() < 30 {
        bail!(
            "expected at least 30 columns in header, found {}",
            fields.len()
        );
    }
    if fields[0] != "run_id" {
        bail!("first column must be 'run_id', found '{}'", fields[0]);
    }
    Ok(())
}

fn parse_row(line: &str) -> Result<BenchmarkRecord> {
    let f = split_csv_row(line);
    if f.len() < 30 {
        bail!("expected at least 30 fields, found {}", f.len());
    }

    Ok(BenchmarkRecord {
        run_id: f[0].to_string(),
        date_utc: f[1].to_string(),
        host_id: f[2].to_string(),
        ram_class_gb: parse_f64(&f[3], "ram_class_gb")?,
        cpu_model: f[4].to_string(),
        backend: f[5].to_string(),
        model_name: f[6].to_string(),
        model_arch: f[7].to_string(),
        total_params_b: parse_f64(&f[8], "total_params_b")?,
        active_params_b: parse_f64(&f[9], "active_params_b")?,
        quantization: f[10].to_string(),
        context_tokens: parse_u64(&f[11], "context_tokens")?,
        output_tokens: parse_u64(&f[12], "output_tokens")?,
        concurrency: parse_u32(&f[13], "concurrency")?,
        compression_profile: f[14].to_string(),
        compression_codec: f[15].to_string(),
        compression_target_pct: parse_u32(&f[16], "compression_target_pct")?,
        zswap_enabled: parse_bool(&f[17], "zswap_enabled")?,
        tokens_per_sec: parse_f64(&f[18], "tokens_per_sec")?,
        ttft_ms: parse_f64(&f[19], "ttft_ms")?,
        p95_token_latency_ms: parse_f64(&f[20], "p95_token_latency_ms")?,
        peak_rss_gb: parse_f64(&f[21], "peak_rss_gb")?,
        swap_used_gb: parse_f64(&f[22], "swap_used_gb")?,
        psi_mem_some_avg10: parse_f64(&f[23], "psi_mem_some_avg10")?,
        psi_mem_full_avg10: parse_f64(&f[24], "psi_mem_full_avg10")?,
        cpu_avg_pct: parse_f64(&f[25], "cpu_avg_pct")?,
        cpu_peak_pct: parse_f64(&f[26], "cpu_peak_pct")?,
        oom_events: parse_u32(&f[27], "oom_events")?,
        run_success: parse_bool(&f[28], "run_success")?,
        notes: f[29].to_string(),
    })
}

fn split_csv_row(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    fields.push(current.trim().to_string());
    fields
}

fn parse_f64(s: &str, field: &str) -> Result<f64> {
    s.parse::<f64>()
        .with_context(|| format!("invalid float for field '{field}': '{s}'"))
}

fn parse_u64(s: &str, field: &str) -> Result<u64> {
    s.parse::<u64>()
        .with_context(|| format!("invalid u64 for field '{field}': '{s}'"))
}

fn parse_u32(s: &str, field: &str) -> Result<u32> {
    s.parse::<u32>()
        .with_context(|| format!("invalid u32 for field '{field}': '{s}'"))
}

fn parse_bool(s: &str, field: &str) -> Result<bool> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => bail!("invalid boolean for field '{field}': '{s}'"),
    }
}
