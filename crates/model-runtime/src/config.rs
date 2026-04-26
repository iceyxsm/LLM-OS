use serde::{Deserialize, Serialize};

/// Configuration for a local model instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Human-readable model name (e.g. "dense-3b-instruct").
    pub name: String,
    /// Path to the model weights file on disk.
    pub model_path: String,
    /// Architecture identifier (e.g. "llama", "mistral", "phi").
    pub architecture: String,
    /// Quantization format (e.g. "Q4_K_M", "Q5_K_S", "F16").
    pub quantization: String,
    /// Maximum context length in tokens.
    pub context_length: u32,
    /// Number of GPU layers to offload (0 = CPU only).
    pub gpu_layers: u32,
    /// Number of threads for CPU inference.
    pub threads: u32,
}

impl ModelConfig {
    /// Create a config for CPU-only inference with sensible defaults.
    pub fn cpu_only(name: impl Into<String>, model_path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            model_path: model_path.into(),
            architecture: "unknown".to_string(),
            quantization: "Q4_K_M".to_string(),
            context_length: 4096,
            gpu_layers: 0,
            threads: 4,
        }
    }
}
