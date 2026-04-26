use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A request to generate text from a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    /// The input prompt.
    pub prompt: String,
    /// Maximum number of tokens to generate.
    pub max_tokens: u32,
    /// Sampling temperature (0.0 = greedy, higher = more random).
    pub temperature: f32,
    /// Optional stop sequences.
    pub stop: Vec<String>,
}

impl InferenceRequest {
    pub fn new(prompt: impl Into<String>, max_tokens: u32) -> Self {
        Self {
            prompt: prompt.into(),
            max_tokens,
            temperature: 0.7,
            stop: vec![],
        }
    }
}

/// The response from an inference request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResponse {
    /// The generated text.
    pub text: String,
    /// Token usage statistics.
    pub usage: TokenUsage,
    /// Time to first token in milliseconds.
    pub ttft_ms: u64,
    /// Total generation time in milliseconds.
    pub total_ms: u64,
}

/// Token usage statistics for a single inference request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Number of tokens in the prompt.
    pub prompt_tokens: u32,
    /// Number of tokens generated.
    pub completion_tokens: u32,
}

impl TokenUsage {
    pub fn total(&self) -> u32 {
        self.prompt_tokens + self.completion_tokens
    }
}

#[derive(Debug, Error)]
pub enum InferenceError {
    #[error("model not loaded: {0}")]
    NotLoaded(String),
    #[error("context length exceeded: requested {requested}, max {max}")]
    ContextLengthExceeded { requested: u32, max: u32 },
    #[error("inference failed: {0}")]
    Failed(String),
    #[error("backend error: {0}")]
    Backend(String),
}
