use async_trait::async_trait;

use crate::config::ModelConfig;
use crate::request::{InferenceError, InferenceRequest, InferenceResponse, TokenUsage};

/// Trait for inference backends.
///
/// Implementations handle model loading and text generation. The trait
/// is designed to support both local backends (llama.cpp, GGML) and
/// remote backends (API calls) through the same interface.
#[async_trait]
pub trait InferenceBackend: Send + Sync {
    /// Load a model from the given configuration.
    async fn load(&mut self, config: &ModelConfig) -> Result<(), InferenceError>;

    /// Check whether a model is currently loaded and ready.
    fn is_loaded(&self) -> bool;

    /// Run inference on the given request.
    async fn infer(&self, request: &InferenceRequest) -> Result<InferenceResponse, InferenceError>;

    /// Unload the current model and free resources.
    async fn unload(&mut self) -> Result<(), InferenceError>;
}

/// A mock backend for testing that returns deterministic responses.
pub struct MockBackend {
    loaded: bool,
    config: Option<ModelConfig>,
    response_text: String,
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            loaded: false,
            config: None,
            response_text: "mock response".to_string(),
        }
    }

    /// Set the text that will be returned by infer().
    pub fn with_response(mut self, text: impl Into<String>) -> Self {
        self.response_text = text.into();
        self
    }

    pub fn loaded_model_name(&self) -> Option<&str> {
        self.config.as_ref().map(|c| c.name.as_str())
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InferenceBackend for MockBackend {
    async fn load(&mut self, config: &ModelConfig) -> Result<(), InferenceError> {
        self.config = Some(config.clone());
        self.loaded = true;
        Ok(())
    }

    fn is_loaded(&self) -> bool {
        self.loaded
    }

    async fn infer(&self, request: &InferenceRequest) -> Result<InferenceResponse, InferenceError> {
        if !self.loaded {
            return Err(InferenceError::NotLoaded("no model loaded".to_string()));
        }

        let config = self.config.as_ref().unwrap();
        let prompt_tokens = (request.prompt.len() / 4) as u32;
        let completion_tokens = request.max_tokens.min(64);

        if prompt_tokens > config.context_length {
            return Err(InferenceError::ContextLengthExceeded {
                requested: prompt_tokens,
                max: config.context_length,
            });
        }

        Ok(InferenceResponse {
            text: self.response_text.clone(),
            usage: TokenUsage {
                prompt_tokens,
                completion_tokens,
            },
            ttft_ms: 10,
            total_ms: 50,
        })
    }

    async fn unload(&mut self) -> Result<(), InferenceError> {
        self.loaded = false;
        self.config = None;
        Ok(())
    }
}
