use std::time::Instant;

use async_trait::async_trait;
use llama_cpp_4::{
    context::LlamaContext, llama_backend::LlamaBackend, model::LlamaModel, sampling::LlamaSampler,
    token::data_array::LlamaTokenDataArray, LlamaContextLoadError, LlamaModelLoadError,
};
use tracing::{debug, info};

use crate::config::ModelConfig;
use crate::request::{InferenceError, InferenceRequest, InferenceResponse, TokenUsage};
use crate::InferenceBackend;

/// An inference backend powered by llama.cpp via the llama-cpp-4 crate.
///
/// This backend loads GGUF model files and runs inference on the CPU
/// (or GPU if compiled with the cuda/metal/vulkan features).
///
/// Enable with the `llama-cpp` feature flag:
/// ```toml
/// llmos-model-runtime = { path = "...", features = ["llama-cpp"] }
/// ```
pub struct LlamaCppBackend {
    backend: Option<LlamaBackend>,
    model: Option<LlamaModel>,
    config: Option<ModelConfig>,
}

impl LlamaCppBackend {
    pub fn new() -> Self {
        Self {
            backend: None,
            model: None,
            config: None,
        }
    }
}

impl Default for LlamaCppBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InferenceBackend for LlamaCppBackend {
    async fn load(&mut self, config: &ModelConfig) -> Result<(), InferenceError> {
        let backend = LlamaBackend::init().map_err(|e| {
            InferenceError::Backend(format!("failed to initialize llama backend: {e}"))
        })?;

        let mut model_params = llama_cpp_4::model::params::LlamaModelParams::default();
        if config.gpu_layers > 0 {
            model_params = model_params.with_n_gpu_layers(config.gpu_layers);
        }

        info!(
            target: "model-runtime::llama",
            model_path = %config.model_path,
            gpu_layers = config.gpu_layers,
            threads = config.threads,
            "loading model"
        );

        let model = LlamaModel::load_from_file(&backend, &config.model_path, &model_params)
            .map_err(|e| {
                InferenceError::NotLoaded(format!(
                    "failed to load model from {}: {e}",
                    config.model_path
                ))
            })?;

        info!(
            target: "model-runtime::llama",
            name = %config.name,
            context_length = config.context_length,
            "model loaded"
        );

        self.backend = Some(backend);
        self.model = Some(model);
        self.config = Some(config.clone());
        Ok(())
    }

    fn is_loaded(&self) -> bool {
        self.model.is_some()
    }

    async fn infer(&self, request: &InferenceRequest) -> Result<InferenceResponse, InferenceError> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| InferenceError::NotLoaded("no model loaded".to_string()))?;
        let config = self.config.as_ref().unwrap();

        let mut ctx_params = llama_cpp_4::context::params::LlamaContextParams::default()
            .with_n_ctx(
                std::num::NonZeroU32::new(config.context_length)
                    .unwrap_or(std::num::NonZeroU32::new(4096).unwrap()),
            );
        if config.threads > 0 {
            ctx_params = ctx_params.with_n_threads(config.threads);
        }

        let mut ctx = model
            .new_context(&self.backend.as_ref().unwrap(), ctx_params)
            .map_err(|e| InferenceError::Backend(format!("failed to create context: {e}")))?;

        let tokens = model
            .str_to_token(&request.prompt, llama_cpp_4::model::AddBos::Always)
            .map_err(|e| InferenceError::Failed(format!("tokenization failed: {e}")))?;

        let prompt_tokens = tokens.len() as u32;
        if prompt_tokens > config.context_length {
            return Err(InferenceError::ContextLengthExceeded {
                requested: prompt_tokens,
                max: config.context_length,
            });
        }

        let started = Instant::now();
        let mut ttft_ms = 0u64;

        // Feed prompt tokens
        let mut batch =
            llama_cpp_4::llama_batch::LlamaBatch::new(config.context_length as usize, 1);
        for (i, &token) in tokens.iter().enumerate() {
            let is_last = i == tokens.len() - 1;
            batch
                .add(token, i as i32, &[0], is_last)
                .map_err(|e| InferenceError::Failed(format!("batch add failed: {e}")))?;
        }

        ctx.decode(&mut batch)
            .map_err(|e| InferenceError::Failed(format!("prompt decode failed: {e}")))?;

        ttft_ms = started.elapsed().as_millis() as u64;

        // Generate tokens
        let mut output_text = String::new();
        let mut completion_tokens = 0u32;
        let mut n_cur = tokens.len();

        let sampler = LlamaSampler::chain_simple([
            LlamaSampler::temp(request.temperature),
            LlamaSampler::dist(42),
        ]);

        for _ in 0..request.max_tokens {
            let logits_token = sampler.sample(&ctx, -1);

            if model.is_eog_token(logits_token) {
                break;
            }

            let piece = model
                .token_to_str(logits_token, llama_cpp_4::token::LlamaTokenAttr::all())
                .unwrap_or_default();

            // Check stop sequences
            output_text.push_str(&piece);
            completion_tokens += 1;

            if request.stop.iter().any(|stop| output_text.ends_with(stop)) {
                // Trim the stop sequence from output
                for stop in &request.stop {
                    if output_text.ends_with(stop) {
                        let new_len = output_text.len() - stop.len();
                        output_text.truncate(new_len);
                        break;
                    }
                }
                break;
            }

            // Prepare next batch
            batch.clear();
            batch
                .add(logits_token, n_cur as i32, &[0], true)
                .map_err(|e| InferenceError::Failed(format!("batch add failed: {e}")))?;
            n_cur += 1;

            ctx.decode(&mut batch)
                .map_err(|e| InferenceError::Failed(format!("decode failed: {e}")))?;
        }

        let total_ms = started.elapsed().as_millis() as u64;

        debug!(
            target: "model-runtime::llama",
            prompt_tokens = prompt_tokens,
            completion_tokens = completion_tokens,
            ttft_ms = ttft_ms,
            total_ms = total_ms,
            "inference complete"
        );

        Ok(InferenceResponse {
            text: output_text,
            usage: TokenUsage {
                prompt_tokens,
                completion_tokens,
            },
            ttft_ms,
            total_ms,
        })
    }

    async fn unload(&mut self) -> Result<(), InferenceError> {
        self.model = None;
        self.config = None;
        self.backend = None;
        info!(target: "model-runtime::llama", "model unloaded");
        Ok(())
    }
}
