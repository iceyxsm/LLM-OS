use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct LlmdConfig {
    pub daemon: DaemonConfig,
    pub model: ModelBackendConfig,
    pub policy: PolicyConfig,
    pub audit: AuditConfig,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DaemonConfig {
    pub name: String,
    pub listen: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelBackendConfig {
    pub backend: String,
    #[serde(default)]
    pub model_path: String,
    #[serde(default = "default_model_name")]
    pub name: String,
    #[serde(default = "default_context_length")]
    pub context_length: u32,
    #[serde(default)]
    pub gpu_layers: u32,
    #[serde(default = "default_threads")]
    pub threads: u32,
    #[serde(default = "default_quantization")]
    pub quantization: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolicyConfig {
    pub endpoint: String,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_max_attempts")]
    pub max_attempts: usize,
    #[serde(default = "default_backoff_initial_ms")]
    pub backoff_initial_ms: u64,
    #[serde(default = "default_backoff_max_ms")]
    pub backoff_max_ms: u64,
    #[serde(default = "default_breaker_threshold")]
    pub breaker_threshold: u32,
    #[serde(default = "default_breaker_cooldown_secs")]
    pub breaker_cooldown_secs: u64,
    #[serde(default = "default_health_interval_secs")]
    pub health_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuditConfig {
    pub sink: String,
    #[serde(default)]
    pub jsonl_path: String,
    #[serde(default = "default_rotate_max_bytes")]
    pub rotate_max_bytes: u64,
    #[serde(default = "default_rotate_max_files")]
    pub rotate_max_files: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsConfig {
    pub listen: String,
}

pub fn load_config(path: &Path) -> Result<LlmdConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config at {}", path.display()))?;
    let config: LlmdConfig =
        toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(config)
}

fn default_model_name() -> String {
    "none".to_string()
}
fn default_context_length() -> u32 {
    4096
}
fn default_threads() -> u32 {
    4
}
fn default_quantization() -> String {
    "Q4_K_M".to_string()
}
fn default_timeout_secs() -> u64 {
    2
}
fn default_max_attempts() -> usize {
    3
}
fn default_backoff_initial_ms() -> u64 {
    100
}
fn default_backoff_max_ms() -> u64 {
    1000
}
fn default_breaker_threshold() -> u32 {
    3
}
fn default_breaker_cooldown_secs() -> u64 {
    5
}
fn default_health_interval_secs() -> u64 {
    30
}
fn default_rotate_max_bytes() -> u64 {
    10 * 1024 * 1024
}
fn default_rotate_max_files() -> usize {
    5
}
