use clap::ValueEnum;
use serde::Serialize;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Serialize)]
pub struct PolicyCheckOutput {
    pub decision: String,
    pub reason: String,
    pub rule_id: Option<String>,
    pub request_id: String,
    pub correlation_id: String,
}

#[derive(Debug, Serialize)]
pub struct PolicyHealthOutput {
    pub status: String,
    pub detail: String,
    pub latency_ms: u128,
    pub request_id: String,
    pub correlation_id: String,
}

pub fn render_json<T: Serialize + ?Sized>(value: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(value)?)
}

pub fn render_lines(lines: &[String]) -> String {
    lines.join("\n")
}
